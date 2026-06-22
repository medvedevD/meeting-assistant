use std::sync::Arc;

use async_trait::async_trait;
use meeting_adapters::settings_store::PersistedSettings;
use meeting_adapters::{
    build_llm, probe_llm, resolve_transcription_model_path, whisper_backend, JsonSettingsStore,
    ProviderKind, SecretStatus, TranscriberPrefs, VaultState,
};
use meeting_api::SettingsService;
use serde_json::{json, Value};

use crate::container::SettingsHandles;

/// Concrete [`SettingsService`] over the JSON settings store, the OS keyring,
/// and the live adapter handles. `update`/`set_secret` hot-apply changes:
/// LLM via `SwappableLlm`, transcriber prefs/model in place, prompts dir swap.
pub struct AppSettingsService {
    handles: SettingsHandles,
}

impl AppSettingsService {
    pub fn new(handles: SettingsHandles) -> Self {
        Self { handles }
    }

    fn store(&self) -> &Arc<JsonSettingsStore> {
        &self.handles.settings_store
    }

    /// Rebuild the active LLM from the given settings + effective key.
    fn rebuild_llm(&self, settings: &PersistedSettings) {
        let active = settings.llm.active;
        let key = self.handles.secrets.effective_key(active.as_str());
        let cfg = settings.llm.resolve(active, key);
        self.handles.llm.set(build_llm(&cfg));
    }

    /// Apply every hot-swappable field. `db`/`recordings` path changes are
    /// restart-required and intentionally not applied here.
    async fn apply(&self, settings: &PersistedSettings) {
        self.handles.transcriber.set_prefs(TranscriberPrefs::new(
            settings.transcriber.language.clone(),
            settings.transcriber.beam_size,
            settings.transcriber.n_threads,
            settings.transcriber.use_gpu,
        ));
        self.handles
            .transcriber
            .set_model_resolution(resolve_transcription_model_path(settings))
            .await;
        if let Some(prompts) = &settings.paths.prompts {
            self.handles.templates.set_dir(prompts);
        }
        self.rebuild_llm(settings);
    }
}

/// Project the secret store's at-rest location onto the sanitized snapshot so
/// the UI can disclose it honestly. `mechanism`/`path` are mutually exclusive:
/// a keystore reports its mechanism, a file backend reports its path. Phase 1
/// ships `keyring` and `plaintext`; `vault`/`locked` arrive in a later phase.
fn secret_storage_view(status: SecretStatus) -> Value {
    match status {
        SecretStatus::Keyring {
            mechanism,
            pending_vault,
        } => json!({
            "kind": "keyring",
            "state": "ready",
            "mechanism": mechanism.as_id(),
            "mechanism_detail": Value::Null,
            "path": Value::Null,
            // An orphaned vault the user can fold into the keystore.
            "pending_migration": if pending_vault { json!("vault") } else { Value::Null },
        }),
        SecretStatus::Vault { state, path } => json!({
            "kind": "vault",
            "state": match state {
                VaultState::Locked => "locked",
                VaultState::Unlocked => "ready",
            },
            "mechanism": Value::Null,
            "mechanism_detail": Value::Null,
            "path": path.display().to_string(),
        }),
        SecretStatus::Plaintext { path } => json!({
            "kind": "plaintext",
            "state": "ready",
            "mechanism": Value::Null,
            "mechanism_detail": Value::Null,
            "path": path.display().to_string(),
        }),
    }
}

fn provider_view(
    cfg: &meeting_adapters::settings_store::ProviderCfg,
    has_key: bool,
    key_hint: Value,
) -> Value {
    json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "base_url": cfg.base_url,
        "has_key": has_key,
        // Non-secret fingerprint so the UI can show *which* key is stored
        // (last 4 chars + length) without ever sending the key itself.
        "key_hint": key_hint,
    })
}

/// A safe, non-reversible fingerprint of a stored key: its last 4 characters
/// and total length. `null` when no key is set.
fn key_hint_value(stored: Option<&str>) -> Value {
    match stored {
        Some(k) if !k.is_empty() => {
            let len = k.chars().count();
            let last4: String = k.chars().skip(len.saturating_sub(4)).collect();
            json!({ "last4": last4, "len": len })
        }
        _ => Value::Null,
    }
}

/// Build a provider view for a key-bearing provider, reading the stored key
/// once to derive both `has_key` and the fingerprint.
fn keyed_provider_view(
    secrets: &meeting_adapters::KeyringSecretStore,
    provider: &str,
    cfg: &meeting_adapters::settings_store::ProviderCfg,
) -> Value {
    let stored = secrets.get(provider).filter(|s| !s.is_empty());
    provider_view(cfg, stored.is_some(), key_hint_value(stored.as_deref()))
}

#[async_trait]
impl SettingsService for AppSettingsService {
    fn snapshot(&self) -> Value {
        let s = self.store().load();
        let secrets = &self.handles.secrets;
        let llm = &s.llm;
        json!({
            "paths": {
                "model": s.paths.model,
                "models_dir": s.paths.models_dir,
                "db": s.paths.db,
                "meetings_dir": s.paths.meetings_dir,
                "prompts": s.paths.prompts,
            },
            "recording": {
                "source": s.recording.source,
                "echo_cancel": s.recording.echo_cancel,
            },
            "default_template": s.default_template,
            "transcriber": {
                "language": s.transcriber.language,
                "beam_size": s.transcriber.beam_size,
                "n_threads": s.transcriber.n_threads,
                "use_gpu": s.transcriber.use_gpu,
                "model_source": s.transcriber.model_source,
                "model_id": s.transcriber.model_id,
                "custom_model_path": s.transcriber.custom_model_path,
                "custom_models": s.transcriber.custom_models,
                "model_path": s.transcriber.custom_model_path,
                // Read-only: compiled Whisper backend (cpu/metal/cuda/vulkan).
                // Ignored on PUT (unknown to PersistedSettings). The UI offers the
                // GPU toggle iff this is not "cpu". See ADR-010.
                "backend": whisper_backend(),
            },
            "llm": {
                "active": llm.active.as_str(),
                // One `get` per provider: derive both has_key and the fingerprint
                // from the same read (avoids a second keyring/vault round-trip).
                "anthropic": keyed_provider_view(secrets, "anthropic", &llm.anthropic),
                "openai": keyed_provider_view(secrets, "openai", &llm.openai),
                "gemini": keyed_provider_view(secrets, "gemini", &llm.gemini),
                "mistral": keyed_provider_view(secrets, "mistral", &llm.mistral),
                "ollama": provider_view(&llm.ollama, true, Value::Null),
            },
            "secret_storage": secret_storage_view(secrets.status()),
        })
    }

    async fn update(&self, body: Value) -> Result<Value, String> {
        let settings: PersistedSettings =
            serde_json::from_value(body).map_err(|e| format!("invalid settings: {e}"))?;
        let settings = settings.normalize();
        self.store()
            .save(settings.clone())
            .map_err(|e| format!("failed to persist settings: {e}"))?;
        self.apply(&settings).await;
        Ok(self.snapshot())
    }

    async fn set_secret(&self, provider: String, value: Option<String>) -> Result<(), String> {
        let kind = ProviderKind::parse(&provider)
            .ok_or_else(|| format!("unknown provider: {provider}"))?;
        self.handles
            .secrets
            .set(kind.as_str(), value.as_deref().unwrap_or(""))
            .map_err(|e| format!("failed to store secret: {e}"))?;
        // If the changed key belongs to the active provider, rebuild it now.
        let settings = self.store().load();
        if settings.llm.active == kind {
            self.rebuild_llm(&settings);
        }
        Ok(())
    }

    async fn test_provider(&self, provider: String) -> Result<(), String> {
        let kind = ProviderKind::parse(&provider)
            .ok_or_else(|| format!("unknown provider: {provider}"))?;
        let settings = self.store().load();
        let key = self.handles.secrets.effective_key(kind.as_str());
        if kind.needs_key() && key.is_empty() {
            return Err("no API key configured".to_string());
        }
        let cfg = settings.llm.resolve(kind, key);
        probe_llm(&cfg).await.map_err(|e| e.to_string())
    }

    async fn protect_secret_store(&self, passphrase: String) -> Result<(), String> {
        self.handles
            .secrets
            .protect_with_passphrase(&passphrase)
            .map_err(|e| e.to_string())
        // Keys are unchanged (same values, now encrypted) — no LLM rebuild.
    }

    async fn unlock_secret_store(&self, passphrase: String) -> Result<(), String> {
        self.handles
            .secrets
            .unlock(&passphrase)
            .map_err(|e| e.to_string())?;
        // Keys became readable — rebuild the active LLM so it picks them up.
        self.rebuild_llm(&self.store().load());
        Ok(())
    }

    async fn lock_secret_store(&self) -> Result<(), String> {
        self.handles.secrets.lock().map_err(|e| e.to_string())?;
        // Keys are no longer readable — rebuild so the active LLM reflects that.
        self.rebuild_llm(&self.store().load());
        Ok(())
    }

    async fn change_secret_passphrase(&self, old: String, new: String) -> Result<(), String> {
        self.handles
            .secrets
            .change_passphrase(&old, &new)
            .map_err(|e| e.to_string())
    }

    async fn migrate_secret_store_to_keyring(&self, passphrase: String) -> Result<(), String> {
        self.handles
            .secrets
            .migrate_vault_to_keyring(&passphrase)
            .map_err(|e| e.to_string())?;
        // Keys now resolve from the keystore — rebuild the active LLM.
        self.rebuild_llm(&self.store().load());
        Ok(())
    }

    async fn reset_secret_store(&self) -> Result<(), String> {
        self.handles
            .secrets
            .reset_store()
            .map_err(|e| e.to_string())?;
        // Keys are gone — rebuild so the active LLM reflects that.
        self.rebuild_llm(&self.store().load());
        Ok(())
    }
}
