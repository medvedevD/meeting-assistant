use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

const SERVICE: &str = "meeting-assistant";

/// Stores per-provider API keys. Prefers the OS keyring; if the keyring is
/// unavailable (e.g. an unsigned macOS dev build that cannot reach the
/// Keychain), transparently falls back to a `0600` JSON file. An environment
/// variable, when set, always overrides stored values (read-only).
pub struct KeyringSecretStore {
    /// `true` when a sentinel round-trip against the OS keyring succeeded.
    keyring_ok: bool,
    fallback_path: PathBuf,
    /// In-memory mirror of the fallback file (only used when `!keyring_ok`).
    file_cache: Mutex<BTreeMap<String, String>>,
}

impl KeyringSecretStore {
    /// Open the store at the default fallback location and detect keyring health.
    pub fn open_default() -> Self {
        let fallback_path = config_dir().join("meeting-assistant/secrets.json");
        Self::open(fallback_path)
    }

    pub fn open(fallback_path: PathBuf) -> Self {
        // Escape hatch (locked-down systems, hermetic tests): force the file
        // fallback and never touch the OS keyring.
        let disabled = std::env::var_os("MEETING_ASSISTANT_KEYRING_DISABLE").is_some();
        let keyring_ok = !disabled && probe_keyring();
        if !keyring_ok {
            tracing::warn!(
                "OS keyring unavailable — API keys will be stored unencrypted at {fallback_path:?} (mode 0600)"
            );
        }
        let file_cache = Mutex::new(load_fallback(&fallback_path));
        Self { keyring_ok, fallback_path, file_cache }
    }

    /// `true` when secrets are persisted in plaintext (keyring unavailable).
    pub fn is_using_fallback(&self) -> bool {
        !self.keyring_ok
    }

    /// Resolve the effective key for a provider: env override → stored value.
    /// Returns an empty string when nothing is set.
    pub fn effective_key(&self, provider: &str) -> String {
        self.get(provider).unwrap_or_default()
    }

    /// `true` when a non-empty key is available (from env or storage).
    pub fn has_key(&self, provider: &str) -> bool {
        !self.effective_key(provider).is_empty()
    }

    /// Read the stored key (env override first). `None` when unset/empty.
    pub fn get(&self, provider: &str) -> Option<String> {
        if let Some(v) = env_override(provider) {
            if !v.is_empty() {
                return Some(v);
            }
        }
        let stored = if self.keyring_ok {
            keyring_get(provider)
        } else {
            self.file_cache.lock().unwrap().get(provider).cloned()
        };
        stored.filter(|s| !s.is_empty())
    }

    /// Persist a key. An empty value deletes it.
    pub fn set(&self, provider: &str, value: &str) -> std::io::Result<()> {
        if value.is_empty() {
            return self.delete(provider);
        }
        if self.keyring_ok {
            keyring_set(provider, value)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        } else {
            let mut cache = self.file_cache.lock().unwrap();
            cache.insert(provider.to_string(), value.to_string());
            write_fallback(&self.fallback_path, &cache)
        }
    }

    /// Remove a stored key (no-op if absent). Does not touch env overrides.
    pub fn delete(&self, provider: &str) -> std::io::Result<()> {
        if self.keyring_ok {
            keyring_delete(provider);
            Ok(())
        } else {
            let mut cache = self.file_cache.lock().unwrap();
            cache.remove(provider);
            write_fallback(&self.fallback_path, &cache)
        }
    }
}

// ── env override mapping ───────────────────────────────────────────────────────

fn env_override(provider: &str) -> Option<String> {
    let var = match provider {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        "mistral" => "MISTRAL_API_KEY",
        _ => return None,
    };
    std::env::var(var).ok()
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

/// Round-trip a sentinel credential to decide whether the OS keyring is usable.
fn probe_keyring() -> bool {
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

// ── 0600 file fallback ──────────────────────────────────────────────────────────

fn load_fallback(path: &PathBuf) -> BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_fallback(path: &PathBuf, data: &BTreeMap<String, String>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)?;
    set_owner_only(path)
}

#[cfg(unix)]
fn set_owner_only(path: &PathBuf) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &PathBuf) -> std::io::Result<()> {
    Ok(())
}

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn file_store(dir: &std::path::Path) -> KeyringSecretStore {
        // Force the file fallback path regardless of keyring availability so the
        // test is deterministic across CI/dev machines.
        KeyringSecretStore {
            keyring_ok: false,
            fallback_path: dir.join("secrets.json"),
            file_cache: Mutex::new(BTreeMap::new()),
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
    fn env_overrides_storage() {
        let dir = tempdir().unwrap();
        let store = file_store(dir.path());
        store.set("anthropic", "stored").unwrap();
        std::env::set_var("ANTHROPIC_API_KEY", "from-env");
        assert_eq!(store.get("anthropic").as_deref(), Some("from-env"));
        std::env::remove_var("ANTHROPIC_API_KEY");
        assert_eq!(store.get("anthropic").as_deref(), Some("stored"));
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
}
