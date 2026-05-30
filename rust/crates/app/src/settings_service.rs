use std::sync::Arc;

use async_trait::async_trait;
use meeting_adapters::settings_store::PersistedSettings;
use meeting_adapters::{
    build_llm, probe_llm, resolve_transcription_model_path, JsonSettingsStore, ProviderKind,
    TranscriberPrefs,
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

fn provider_view(cfg: &meeting_adapters::settings_store::ProviderCfg, has_key: bool) -> Value {
    json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "base_url": cfg.base_url,
        "has_key": has_key,
    })
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
                "model_source": s.transcriber.model_source,
                "model_id": s.transcriber.model_id,
                "custom_model_path": s.transcriber.custom_model_path,
                "custom_models": s.transcriber.custom_models,
                "model_path": s.transcriber.custom_model_path,
            },
            "llm": {
                "active": llm.active.as_str(),
                "anthropic": provider_view(&llm.anthropic, secrets.has_key("anthropic")),
                "openai": provider_view(&llm.openai, secrets.has_key("openai")),
                "gemini": provider_view(&llm.gemini, secrets.has_key("gemini")),
                "mistral": provider_view(&llm.mistral, secrets.has_key("mistral")),
                "ollama": provider_view(&llm.ollama, true),
            },
            "secrets_fallback": secrets.is_using_fallback(),
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
}
