use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const SERVICE: &str = "meeting-assistant";

// Argon2id parameters (OWASP-ish baseline). Stored in the vault file so they can
// be strengthened later without breaking existing vaults.
const VAULT_M_COST: u32 = 19_456; // 19 MiB
const VAULT_T_COST: u32 = 2;
const VAULT_P_COST: u32 = 1;

/// Where secrets are stored at rest — projected onto the settings snapshot so
/// the UI can disclose it honestly.
pub enum SecretStatus {
    /// Backed by the OS keystore (encrypted at rest, OS-mediated access).
    /// `pending_vault` is true when an orphaned passphrase vault still sits on
    /// disk — the user can migrate its keys into the keystore.
    Keyring {
        mechanism: KeyringMechanism,
        pending_vault: bool,
    },
    /// Passphrase-encrypted vault file. `Locked` until the user unlocks it.
    Vault { state: VaultState, path: PathBuf },
    /// Plaintext `0600` fallback file (no keystore, no vault).
    Plaintext { path: PathBuf },
}

/// Runtime state of the passphrase vault.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    /// Encrypted on disk; the passphrase has not been supplied this session.
    Locked,
    /// Decrypted in memory; keys are readable and writable.
    Unlocked,
}

/// Errors from the secret-store lifecycle operations (unlock/lock/etc.).
#[derive(Debug)]
pub enum SecretError {
    /// The vault is locked — supply the passphrase first.
    Locked,
    /// The supplied passphrase did not decrypt the vault.
    WrongPassphrase,
    /// The operation does not apply to the current backend.
    NotApplicable,
    Io(std::io::Error),
}

impl std::fmt::Display for SecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretError::Locked => write!(f, "secret store is locked"),
            SecretError::WrongPassphrase => write!(f, "incorrect passphrase"),
            SecretError::NotApplicable => {
                write!(f, "operation not applicable to the current secret store")
            }
            SecretError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SecretError {}

/// The concrete OS keystore in use, derived from the compiled backend. `None`
/// means no real keystore is compiled in for this target (keyring would resolve
/// to an in-memory mock — treated as unavailable, see [`probe_keyring`]).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyringMechanism {
    AppleKeychain,
    WindowsCredentialManager,
    SecretService,
    None,
}

impl KeyringMechanism {
    /// Stable id for the wire; QML maps these to localized labels.
    pub fn as_id(self) -> &'static str {
        match self {
            KeyringMechanism::AppleKeychain => "apple_keychain",
            KeyringMechanism::WindowsCredentialManager => "windows_credential_manager",
            KeyringMechanism::SecretService => "secret_service",
            KeyringMechanism::None => "none",
        }
    }
}

/// The keystore mechanism compiled for this target. macOS/Windows use their
/// native backends; Linux uses Secret Service (the `sync-secret-service`
/// feature, enabled per-target in `Cargo.toml`). Any other target has no real
/// backend.
fn keyring_mechanism() -> KeyringMechanism {
    #[cfg(target_os = "macos")]
    {
        KeyringMechanism::AppleKeychain
    }
    #[cfg(target_os = "windows")]
    {
        KeyringMechanism::WindowsCredentialManager
    }
    #[cfg(target_os = "linux")]
    {
        KeyringMechanism::SecretService
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        KeyringMechanism::None
    }
}

/// The active backend and its in-memory state. Held behind a `Mutex` so the
/// lifecycle operations (unlock/lock/protect) can swap it at runtime.
enum Inner {
    /// OS keystore. Reads/writes go straight to the keyring.
    Keyring,
    /// Plaintext fallback; the map mirrors the `0600` file.
    Plaintext(BTreeMap<String, String>),
    /// Encrypted vault, passphrase not yet supplied.
    VaultLocked(VaultFile),
    /// Encrypted vault, decrypted in memory. `key`/`kdf` re-encrypt on write.
    VaultUnlocked {
        key: Zeroizing<[u8; 32]>,
        kdf: KdfParams,
        secrets: BTreeMap<String, String>,
    },
}

/// Stores per-provider API keys. Prefers the OS keyring; if it is unavailable
/// (e.g. an unsigned macOS dev build, or Linux without a Secret Service), uses
/// either a passphrase-encrypted vault (if one exists) or a `0600` plaintext
/// file. Keys come only from what the user configured — no env override.
pub struct KeyringSecretStore {
    fallback_path: PathBuf,
    vault_path: PathBuf,
    inner: Mutex<Inner>,
}

impl KeyringSecretStore {
    /// Open the store at the default fallback location and detect keyring health.
    pub fn open_default() -> Self {
        let fallback_path = config_dir().join("meeting-assistant/secrets.json");
        Self::open(fallback_path)
    }

    pub fn open(fallback_path: PathBuf) -> Self {
        let vault_path = fallback_path.with_file_name("secrets.vault.json");
        // Escape hatch (locked-down systems, hermetic tests): force the file
        // fallback and never touch the OS keyring.
        let disabled = std::env::var_os("MEETING_ASSISTANT_KEYRING_DISABLE").is_some();
        let keyring_ok = !disabled && probe_keyring();

        let inner = if keyring_ok {
            // A keystore is available again — fold any stale plaintext keys into
            // it and delete the plaintext file (no passphrase needed). A vault
            // requires the passphrase, so it is left for a user-driven migrate.
            import_plaintext_into_keyring(&fallback_path);
            Inner::Keyring
        } else if let Some(vf) = load_vault(&vault_path) {
            tracing::info!("OS keyring unavailable — using passphrase vault at {vault_path:?}");
            Inner::VaultLocked(vf)
        } else {
            tracing::warn!(
                "OS keyring unavailable — API keys will be stored unencrypted at {fallback_path:?} (mode 0600); offer the user a passphrase vault"
            );
            Inner::Plaintext(load_fallback(&fallback_path))
        };

        Self {
            fallback_path,
            vault_path,
            inner: Mutex::new(inner),
        }
    }

    /// Where secrets physically live, for honest UI disclosure.
    pub fn status(&self) -> SecretStatus {
        match &*self.inner.lock().unwrap() {
            Inner::Keyring => SecretStatus::Keyring {
                mechanism: keyring_mechanism(),
                pending_vault: self.vault_path.exists(),
            },
            Inner::Plaintext(_) => SecretStatus::Plaintext {
                path: self.fallback_path.clone(),
            },
            Inner::VaultLocked(_) => SecretStatus::Vault {
                state: VaultState::Locked,
                path: self.vault_path.clone(),
            },
            Inner::VaultUnlocked { .. } => SecretStatus::Vault {
                state: VaultState::Unlocked,
                path: self.vault_path.clone(),
            },
        }
    }

    /// Resolve the stored key for a provider. Returns an empty string when
    /// nothing is set (or the vault is locked).
    pub fn effective_key(&self, provider: &str) -> String {
        self.get(provider).unwrap_or_default()
    }

    /// `true` when a non-empty key is available (from env or storage).
    pub fn has_key(&self, provider: &str) -> bool {
        !self.effective_key(provider).is_empty()
    }

    /// Read the stored key. `None` when unset/empty, or when the vault is
    /// locked. Keys come only from what the user configured (keyring/vault/
    /// plaintext) — there is intentionally no ambient environment override, so
    /// the UI's `has_key` always reflects the real, manageable state.
    pub fn get(&self, provider: &str) -> Option<String> {
        let stored = match &*self.inner.lock().unwrap() {
            Inner::Keyring => keyring_get(provider),
            Inner::Plaintext(m) => m.get(provider).cloned(),
            Inner::VaultLocked(_) => None,
            Inner::VaultUnlocked { secrets, .. } => secrets.get(provider).cloned(),
        };
        stored.filter(|s| !s.is_empty())
    }

    /// Persist a key. An empty value deletes it. Fails if the vault is locked.
    pub fn set(&self, provider: &str, value: &str) -> std::io::Result<()> {
        if value.is_empty() {
            return self.delete(provider);
        }
        match &mut *self.inner.lock().unwrap() {
            Inner::Keyring => keyring_set(provider, value).map_err(io_other),
            Inner::Plaintext(m) => {
                m.insert(provider.to_string(), value.to_string());
                write_fallback(&self.fallback_path, m)
            }
            Inner::VaultLocked(_) => Err(locked_io()),
            Inner::VaultUnlocked { key, kdf, secrets } => {
                secrets.insert(provider.to_string(), value.to_string());
                let vf = encrypt_with_key(key, kdf, secrets)?;
                write_vault_file(&self.vault_path, &vf)
            }
        }
    }

    /// Remove a stored key (no-op if absent).
    pub fn delete(&self, provider: &str) -> std::io::Result<()> {
        match &mut *self.inner.lock().unwrap() {
            Inner::Keyring => {
                keyring_delete(provider);
                Ok(())
            }
            Inner::Plaintext(m) => {
                m.remove(provider);
                write_fallback(&self.fallback_path, m)
            }
            Inner::VaultLocked(_) => Err(locked_io()),
            Inner::VaultUnlocked { key, kdf, secrets } => {
                secrets.remove(provider);
                let vf = encrypt_with_key(key, kdf, secrets)?;
                write_vault_file(&self.vault_path, &vf)
            }
        }
    }

    // ── vault lifecycle ─────────────────────────────────────────────────────

    /// Convert the current plaintext store into a passphrase-encrypted vault,
    /// then leave it unlocked. Removes the plaintext file. No-op-safe to call
    /// with an empty store (creates an empty vault).
    pub fn protect_with_passphrase(&self, passphrase: &str) -> Result<(), SecretError> {
        let mut guard = self.inner.lock().unwrap();
        let secrets = match &*guard {
            Inner::Plaintext(m) => m.clone(),
            _ => return Err(SecretError::NotApplicable),
        };
        let (key, kdf, vf) = seal_vault(passphrase, &secrets)?;
        write_vault_file(&self.vault_path, &vf).map_err(SecretError::Io)?;
        // The keys now live encrypted in the vault; drop the plaintext file.
        let _ = std::fs::remove_file(&self.fallback_path);
        *guard = Inner::VaultUnlocked { key, kdf, secrets };
        Ok(())
    }

    /// Decrypt the vault with `passphrase`. Idempotent if already unlocked.
    pub fn unlock(&self, passphrase: &str) -> Result<(), SecretError> {
        let mut guard = self.inner.lock().unwrap();
        match &*guard {
            Inner::VaultLocked(vf) => {
                let kdf = vf.kdf.clone();
                let (key, secrets) = open_vault(passphrase, vf)?;
                *guard = Inner::VaultUnlocked { key, kdf, secrets };
                Ok(())
            }
            Inner::VaultUnlocked { .. } => Ok(()),
            _ => Err(SecretError::NotApplicable),
        }
    }

    /// Drop the decrypted keys from memory, returning the vault to `Locked`.
    pub fn lock(&self) -> Result<(), SecretError> {
        let mut guard = self.inner.lock().unwrap();
        if matches!(&*guard, Inner::VaultUnlocked { .. }) {
            // Writes persist on every `set`, so the on-disk file is current.
            let vf = load_vault(&self.vault_path)
                .ok_or_else(|| SecretError::Io(io_other("vault file missing")))?;
            *guard = Inner::VaultLocked(vf);
        }
        Ok(())
    }

    /// Re-encrypt the vault under a new passphrase. Verifies `old` against the
    /// on-disk file (so it works whether or not the vault is currently
    /// unlocked) and leaves the vault unlocked.
    pub fn change_passphrase(&self, old: &str, new: &str) -> Result<(), SecretError> {
        let mut guard = self.inner.lock().unwrap();
        if !matches!(&*guard, Inner::VaultLocked(_) | Inner::VaultUnlocked { .. }) {
            return Err(SecretError::NotApplicable);
        }
        let vf = load_vault(&self.vault_path)
            .ok_or_else(|| SecretError::Io(io_other("vault file missing")))?;
        let (_old_key, secrets) = open_vault(old, &vf)?;
        let (key, kdf, new_vf) = seal_vault(new, &secrets)?;
        write_vault_file(&self.vault_path, &new_vf).map_err(SecretError::Io)?;
        *guard = Inner::VaultUnlocked { key, kdf, secrets };
        Ok(())
    }

    /// Decrypt the orphaned vault with `passphrase` and import its keys into the
    /// OS keystore, then delete the vault file. Only valid when a keystore is
    /// active (the typical "keyring came back" case).
    pub fn migrate_vault_to_keyring(&self, passphrase: &str) -> Result<(), SecretError> {
        let guard = self.inner.lock().unwrap();
        if !matches!(&*guard, Inner::Keyring) {
            return Err(SecretError::NotApplicable);
        }
        let vf = load_vault(&self.vault_path)
            .ok_or_else(|| SecretError::Io(io_other("vault file missing")))?;
        let (_key, secrets) = open_vault(passphrase, &vf)?;
        for (provider, value) in &secrets {
            keyring_set(provider, value).map_err(|e| SecretError::Io(io_other(e)))?;
        }
        let _ = std::fs::remove_file(&self.vault_path);
        Ok(())
    }

    /// Discard the secret store entirely — delete both the vault and the
    /// plaintext file. The escape hatch for a forgotten vault passphrase; the
    /// stored keys are lost. Returns to an empty plaintext store (or stays on
    /// the keystore if one is active).
    pub fn reset_store(&self) -> Result<(), SecretError> {
        let mut guard = self.inner.lock().unwrap();
        let on_keyring = matches!(&*guard, Inner::Keyring);
        let _ = std::fs::remove_file(&self.vault_path);
        let _ = std::fs::remove_file(&self.fallback_path);
        *guard = if on_keyring {
            Inner::Keyring
        } else {
            Inner::Plaintext(BTreeMap::new())
        };
        Ok(())
    }
}

// ── keyring backend ─────────────────────────────────────────────────────────────

fn entry_user(provider: &str) -> String {
    format!("api_key.{provider}")
}

fn keyring_get(provider: &str) -> Option<String> {
    let entry = keyring::Entry::new(SERVICE, &entry_user(provider)).ok()?;
    entry.get_password().ok()
}

fn keyring_set(provider: &str, value: &str) -> Result<(), keyring::Error> {
    let entry = keyring::Entry::new(SERVICE, &entry_user(provider))?;
    entry.set_password(value)
}

fn keyring_delete(provider: &str) {
    if let Ok(entry) = keyring::Entry::new(SERVICE, &entry_user(provider)) {
        let _ = entry.delete_credential();
    }
}

/// Best-effort: fold any keys from a stale plaintext fallback file into the OS
/// keystore (without clobbering keys it already holds), then delete the file.
/// Called at startup when the keystore is healthy again. Keeps a lingering
/// plaintext copy from outliving the fallback that produced it.
fn import_plaintext_into_keyring(path: &PathBuf) {
    if !path.exists() {
        return;
    }
    let map = load_fallback(path);
    let mut all_imported = true;
    for (provider, value) in &map {
        if keyring_get(provider).is_none() && keyring_set(provider, value).is_err() {
            all_imported = false;
        }
    }
    // Only drop the plaintext once its keys are safely in the keystore.
    if all_imported {
        let _ = std::fs::remove_file(path);
    }
}

/// Round-trip a sentinel credential to decide whether the OS keyring is usable.
fn probe_keyring() -> bool {
    // No real backend compiled (keyring would silently use an in-memory mock):
    // never report the keystore as usable, or we'd lose keys and lie to the UI.
    if keyring_mechanism() == KeyringMechanism::None {
        return false;
    }
    let user = "__healthcheck__";
    let Ok(entry) = keyring::Entry::new(SERVICE, user) else {
        return false;
    };
    if entry.set_password("ok").is_err() {
        return false;
    }
    let ok = matches!(entry.get_password().as_deref(), Ok("ok"));
    let _ = entry.delete_credential();
    ok
}

// ── passphrase vault (Argon2id + XChaCha20-Poly1305) ────────────────────────────

/// On-disk encrypted vault. `nonce`/`ciphertext`/`kdf.salt` are base64.
#[derive(Serialize, Deserialize)]
struct VaultFile {
    version: u32,
    kdf: KdfParams,
    aead: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct KdfParams {
    alg: String,
    salt: String,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
}

fn derive_key(
    passphrase: &str,
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> std::io::Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(32)).map_err(io_invalid)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut_slice())
        .map_err(io_invalid)?;
    Ok(key)
}

fn encrypt_with_key(
    key: &[u8; 32],
    kdf: &KdfParams,
    secrets: &BTreeMap<String, String>,
) -> std::io::Result<VaultFile> {
    let plaintext = serde_json::to_vec(secrets).map_err(io_invalid)?;
    let mut nonce_bytes = [0u8; 24];
    getrandom::getrandom(&mut nonce_bytes).map_err(io_other)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce_bytes), plaintext.as_ref())
        .map_err(|_| io_other("vault encryption failed"))?;
    Ok(VaultFile {
        version: 1,
        kdf: kdf.clone(),
        aead: "xchacha20poly1305".to_string(),
        nonce: B64.encode(nonce_bytes),
        ciphertext: B64.encode(ciphertext),
    })
}

/// Derived key + its KDF params + the sealed file, as returned by [`seal_vault`].
type Sealed = (Zeroizing<[u8; 32]>, KdfParams, VaultFile);

/// Generate a fresh salt, derive a key from `passphrase`, and seal `secrets`.
fn seal_vault(passphrase: &str, secrets: &BTreeMap<String, String>) -> Result<Sealed, SecretError> {
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|e| SecretError::Io(io_other(e)))?;
    let kdf = KdfParams {
        alg: "argon2id".to_string(),
        salt: B64.encode(salt),
        m_cost: VAULT_M_COST,
        t_cost: VAULT_T_COST,
        p_cost: VAULT_P_COST,
    };
    let key = derive_key(passphrase, &salt, VAULT_M_COST, VAULT_T_COST, VAULT_P_COST)
        .map_err(SecretError::Io)?;
    let vf = encrypt_with_key(&key, &kdf, secrets).map_err(SecretError::Io)?;
    Ok((key, kdf, vf))
}

/// Derived key + the decrypted secret map, as returned by [`open_vault`].
type Opened = (Zeroizing<[u8; 32]>, BTreeMap<String, String>);

/// Derive the key and decrypt `vf`. A wrong passphrase fails the AEAD tag.
fn open_vault(passphrase: &str, vf: &VaultFile) -> Result<Opened, SecretError> {
    let decode = |s: &str| B64.decode(s).map_err(|e| SecretError::Io(io_invalid(e)));
    let salt = decode(&vf.kdf.salt)?;
    let nonce_bytes = decode(&vf.nonce)?;
    let ciphertext = decode(&vf.ciphertext)?;
    if nonce_bytes.len() != 24 {
        return Err(SecretError::Io(io_invalid("vault nonce has wrong length")));
    }
    let key = derive_key(
        passphrase,
        &salt,
        vf.kdf.m_cost,
        vf.kdf.t_cost,
        vf.kdf.p_cost,
    )
    .map_err(SecretError::Io)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&*key));
    let plaintext = cipher
        .decrypt(XNonce::from_slice(&nonce_bytes), ciphertext.as_ref())
        .map_err(|_| SecretError::WrongPassphrase)?;
    let secrets = serde_json::from_slice(&plaintext).map_err(|e| SecretError::Io(io_invalid(e)))?;
    Ok((key, secrets))
}

fn load_vault(path: &PathBuf) -> Option<VaultFile> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn write_vault_file(path: &PathBuf, vf: &VaultFile) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(vf).map_err(io_invalid)?;
    atomic_write_0600(path, json.as_bytes())
}

fn locked_io() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, "secret store is locked")
}

// ── 0600 file fallback ──────────────────────────────────────────────────────────

fn load_fallback(path: &PathBuf) -> BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_fallback(path: &PathBuf, data: &BTreeMap<String, String>) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(data).map_err(io_invalid)?;
    atomic_write_0600(path, json.as_bytes())
}

/// Write `bytes` to `path` atomically and owner-only: a sibling temp file is
/// created `0600` from the start, fsynced, then renamed over the target. This
/// closes the world-readable window of `fs::write` + chmod and avoids torn
/// writes on crash. The containing directory is restricted to `0700`.
fn atomic_write_0600(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        set_dir_owner_only(parent)?;
    }
    let mut tmp = path.clone().into_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    write_owner_only(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Create `path` (replacing any stale temp) with `0600` at creation time and
/// fsync it, so the secrets never exist on disk world-readable, even briefly.
#[cfg(unix)]
fn write_owner_only(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    // O_EXCL guarantees we own the inode and its `0600` mode (no umask race).
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

#[cfg(not(unix))]
fn write_owner_only(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

/// Restrict the directory holding the secret files to the owner (`0700`).
#[cfg(unix)]
fn set_dir_owner_only(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_dir_owner_only(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}

fn io_other<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

fn io_invalid<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn file_store(dir: &std::path::Path) -> KeyringSecretStore {
        // Force the plaintext fallback regardless of keyring availability so the
        // test is deterministic across CI/dev machines.
        KeyringSecretStore {
            fallback_path: dir.join("secrets.json"),
            vault_path: dir.join("secrets.vault.json"),
            inner: Mutex::new(Inner::Plaintext(BTreeMap::new())),
        }
    }

    #[test]
    fn set_get_delete_roundtrip_via_file() {
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());

        assert!(!store.has_key("openai"));
        store.set("openai", "sk-123").unwrap();
        assert_eq!(store.get("openai").as_deref(), Some("sk-123"));
        assert!(store.has_key("openai"));

        store.delete("openai").unwrap();
        assert!(!store.has_key("openai"));
    }

    #[test]
    fn empty_value_deletes() {
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());
        store.set("mistral", "k").unwrap();
        store.set("mistral", "").unwrap();
        assert!(!store.has_key("mistral"));
    }

    #[test]
    fn ambient_env_var_does_not_override_storage() {
        // The store deliberately ignores ANTHROPIC_API_KEY & friends: a desktop
        // app must use exactly the key the user configured (and the UI shows),
        // not an ambient env var leaking from a parent process.
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());
        store.set("anthropic", "stored").unwrap();
        std::env::set_var("ANTHROPIC_API_KEY", "from-env");
        assert_eq!(store.get("anthropic").as_deref(), Some("stored"));
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[cfg(unix)]
    #[test]
    fn fallback_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());
        store.set("openai", "k").unwrap();
        let mode = std::fs::metadata(dir.path().join("secrets.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn vault_protect_unlock_lock_roundtrip() {
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());
        store.set("openai", "sk-1").unwrap();

        // Protect → vault is created, unlocked, plaintext file removed.
        store.protect_with_passphrase("hunter2").unwrap();
        assert!(matches!(
            store.status(),
            SecretStatus::Vault {
                state: VaultState::Unlocked,
                ..
            }
        ));
        assert!(!dir.path().join("secrets.json").exists());
        assert_eq!(store.get("openai").as_deref(), Some("sk-1"));

        // Writes while unlocked persist (re-encrypted).
        store.set("anthropic", "sk-a").unwrap();

        // Lock → keys disappear from memory.
        store.lock().unwrap();
        assert!(matches!(
            store.status(),
            SecretStatus::Vault {
                state: VaultState::Locked,
                ..
            }
        ));
        assert!(store.get("openai").is_none());
        assert!(store.set("gemini", "x").is_err());

        // Wrong passphrase rejected; correct one restores access.
        assert!(matches!(
            store.unlock("nope"),
            Err(SecretError::WrongPassphrase)
        ));
        store.unlock("hunter2").unwrap();
        assert_eq!(store.get("openai").as_deref(), Some("sk-1"));
        assert_eq!(store.get("anthropic").as_deref(), Some("sk-a"));
    }

    #[test]
    fn vault_change_passphrase() {
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());
        store.set("openai", "sk-1").unwrap();
        store.protect_with_passphrase("old-pass").unwrap();

        assert!(matches!(
            store.change_passphrase("wrong", "new-pass"),
            Err(SecretError::WrongPassphrase)
        ));
        store.change_passphrase("old-pass", "new-pass").unwrap();

        store.lock().unwrap();
        assert!(store.unlock("old-pass").is_err());
        store.unlock("new-pass").unwrap();
        assert_eq!(store.get("openai").as_deref(), Some("sk-1"));
    }

    #[test]
    fn reset_clears_vault_back_to_empty_plaintext() {
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());
        store.set("openai", "sk-1").unwrap();
        store.protect_with_passphrase("pw").unwrap();
        store.lock().unwrap();

        // Forgotten passphrase → reset wipes the vault and starts over.
        store.reset_store().unwrap();
        assert!(!dir.path().join("secrets.vault.json").exists());
        assert!(matches!(store.status(), SecretStatus::Plaintext { .. }));
        assert!(!store.has_key("openai"));

        // The store is usable again afterwards.
        store.set("anthropic", "sk-a").unwrap();
        assert_eq!(store.get("anthropic").as_deref(), Some("sk-a"));
    }

    #[cfg(unix)]
    #[test]
    fn vault_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());
        store.protect_with_passphrase("pw").unwrap();
        let mode = std::fs::metadata(dir.path().join("secrets.vault.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
