use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Deserializer, Serialize};

use crate::llm::{LlmConfig, ProviderKind};

/// Persisted user settings. All fields are optional to allow partial saves and forward-compat.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedSettings {
    pub paths: PersistedPaths,
    /// **Deprecated** plaintext key from the pre-keyring era. Read only so the
    /// composition layer can migrate it into the OS keyring, then clear it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic_api_key: Option<String>,
    pub recording: RecordingPrefs,
    pub default_template: Option<String>,
    /// Names of bundled templates the user deliberately deleted. Startup template
    /// backfill skips these so a deletion is not undone on the next launch. See
    /// [`meeting_core::usecases::backfill_templates`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_bundled_templates: Vec<String>,
    #[serde(default)]
    pub transcriber: PersistedTranscriberPrefs,
    #[serde(default)]
    pub llm: LlmPrefs,
}

impl PersistedSettings {
    pub fn normalize(mut self) -> Self {
        self.transcriber.normalize(self.paths.model.clone());
        self
    }

    pub fn effective_models_dir(&self) -> PathBuf {
        self.paths
            .models_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(default_models_dir)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PersistedTranscriberPrefs {
    /// BCP-47 language code or "auto". Default: "ru".
    pub language: String,
    /// Beam size for decoding. 1 = Greedy (fastest), 2–5 = BeamSearch. Default: 1.
    #[serde(default = "default_beam_size")]
    pub beam_size: u32,
    /// CPU threads for inference. 0 = auto (physical cores). Default: 0.
    #[serde(default)]
    pub n_threads: u32,
    /// Offload to the compiled GPU backend when available. No effect on CPU-only
    /// builds. Default: true. See `whisper_backend()` / ADR-006.
    #[serde(default = "default_use_gpu")]
    pub use_gpu: bool,
    /// Managed catalog model or advanced custom path. Old `model_path` JSON is
    /// accepted as an alias for `custom_model_path` and normalized to
    /// `model_source = custom_path`.
    #[serde(default)]
    pub model_source: TranscriptionModelSource,
    #[serde(
        default,
        deserialize_with = "empty_string_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub model_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "empty_string_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_models: Vec<CustomTranscriptionModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomTranscriptionModel {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Deserialize)]
struct PersistedTranscriberPrefsWire {
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_beam_size")]
    beam_size: u32,
    #[serde(default)]
    n_threads: u32,
    #[serde(default = "default_use_gpu")]
    use_gpu: bool,
    #[serde(default)]
    model_source: TranscriptionModelSource,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    model_id: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    custom_model_path: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    model_path: Option<String>,
    #[serde(default)]
    custom_models: Vec<CustomTranscriptionModel>,
}

impl<'de> Deserialize<'de> for PersistedTranscriberPrefs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = PersistedTranscriberPrefsWire::deserialize(deserializer)?;
        Ok(Self {
            language: wire.language,
            beam_size: wire.beam_size,
            n_threads: wire.n_threads,
            use_gpu: wire.use_gpu,
            model_source: wire.model_source,
            model_id: wire.model_id,
            custom_model_path: wire.custom_model_path.or(wire.model_path),
            custom_models: wire.custom_models,
        })
    }
}

fn default_beam_size() -> u32 {
    1
}

fn default_language() -> String {
    "ru".to_string()
}

fn default_use_gpu() -> bool {
    true
}

fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(
        Option::<String>::deserialize(deserializer)?.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }),
    )
}

impl Default for PersistedTranscriberPrefs {
    fn default() -> Self {
        Self {
            language: "ru".into(),
            beam_size: 1,
            n_threads: 0,
            use_gpu: true,
            model_source: TranscriptionModelSource::Managed,
            model_id: None,
            custom_model_path: None,
            custom_models: Vec::new(),
        }
    }
}

impl PersistedTranscriberPrefs {
    fn normalize(&mut self, legacy_paths_model: Option<String>) {
        if self.custom_model_path.is_none() {
            self.custom_model_path = legacy_paths_model.and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            });
        }
        if self.custom_model_path.is_some() && self.model_id.is_none() {
            self.model_source = TranscriptionModelSource::CustomPath;
        }
        if let Some(path) = self.custom_model_path.clone() {
            self.ensure_custom_model_for_path(&path);
        }
    }

    fn ensure_custom_model_for_path(&mut self, path: &str) {
        if self.custom_models.iter().any(|model| model.path == path) {
            return;
        }
        self.custom_models.push(CustomTranscriptionModel {
            id: custom_model_id(path),
            name: custom_model_name(path),
            path: path.to_string(),
            description: None,
        });
    }
}

fn custom_model_id(path: &str) -> String {
    let mut id = String::from("custom-");
    for b in path.as_bytes() {
        id.push_str(&format!("{b:02x}"));
    }
    id
}

fn custom_model_name(path: &str) -> String {
    let filename = PathBuf::from(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Custom model")
        .to_string();
    filename
        .strip_prefix("ggml-")
        .unwrap_or(&filename)
        .strip_suffix(".bin")
        .unwrap_or_else(|| filename.strip_prefix("ggml-").unwrap_or(&filename))
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionModelSource {
    Managed,
    CustomPath,
}

impl Default for TranscriptionModelSource {
    fn default() -> Self {
        Self::Managed
    }
}

// ── LLM provider preferences ───────────────────────────────────────────────────

/// Per-provider configuration. All five providers are stored simultaneously so
/// switching the active provider never loses the others' settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPrefs {
    /// Currently selected provider.
    #[serde(default = "default_provider")]
    pub active: ProviderKind,
    #[serde(default = "anthropic_cfg")]
    pub anthropic: ProviderCfg,
    #[serde(default = "openai_cfg")]
    pub openai: ProviderCfg,
    #[serde(default = "gemini_cfg")]
    pub gemini: ProviderCfg,
    #[serde(default = "mistral_cfg")]
    pub mistral: ProviderCfg,
    #[serde(default = "ollama_cfg")]
    pub ollama: ProviderCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCfg {
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// `None` = use the provider's default endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

fn default_max_tokens() -> u32 {
    4096
}
fn default_provider() -> ProviderKind {
    ProviderKind::Anthropic
}

fn cfg_for(kind: ProviderKind) -> ProviderCfg {
    ProviderCfg {
        model: kind.default_model().to_string(),
        max_tokens: 4096,
        base_url: None,
    }
}
fn anthropic_cfg() -> ProviderCfg {
    cfg_for(ProviderKind::Anthropic)
}
fn openai_cfg() -> ProviderCfg {
    cfg_for(ProviderKind::Openai)
}
fn gemini_cfg() -> ProviderCfg {
    cfg_for(ProviderKind::Gemini)
}
fn mistral_cfg() -> ProviderCfg {
    cfg_for(ProviderKind::Mistral)
}
fn ollama_cfg() -> ProviderCfg {
    cfg_for(ProviderKind::Ollama)
}

impl Default for LlmPrefs {
    fn default() -> Self {
        Self {
            active: ProviderKind::Anthropic,
            anthropic: anthropic_cfg(),
            openai: openai_cfg(),
            gemini: gemini_cfg(),
            mistral: mistral_cfg(),
            ollama: ollama_cfg(),
        }
    }
}

impl LlmPrefs {
    pub fn cfg(&self, kind: ProviderKind) -> &ProviderCfg {
        match kind {
            ProviderKind::Anthropic => &self.anthropic,
            ProviderKind::Openai => &self.openai,
            ProviderKind::Gemini => &self.gemini,
            ProviderKind::Mistral => &self.mistral,
            ProviderKind::Ollama => &self.ollama,
        }
    }

    /// Resolve a runnable [`LlmConfig`] for the given provider, filling the
    /// effective API key (env override or stored) supplied by the caller.
    pub fn resolve(&self, kind: ProviderKind, api_key: String) -> LlmConfig {
        let cfg = self.cfg(kind);
        LlmConfig {
            kind,
            model: cfg.model.clone(),
            max_tokens: cfg.max_tokens,
            base_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| kind.default_base_url().to_string()),
            api_key,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedPaths {
    pub model: Option<String>,
    pub models_dir: Option<String>,
    pub db: Option<String>,
    #[serde(alias = "recordings")]
    pub meetings_dir: Option<String>,
    pub prompts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingPrefs {
    pub source: String,
    pub echo_cancel: bool,
}

impl Default for RecordingPrefs {
    fn default() -> Self {
        Self {
            source: "mic".into(),
            echo_cancel: false,
        }
    }
}

// ── JsonSettingsStore ─────────────────────────────────────────────────────────

pub struct JsonSettingsStore {
    path: PathBuf,
    data: Mutex<PersistedSettings>,
}

impl JsonSettingsStore {
    /// Open (or create) the settings file. If the file does not exist, defaults are used.
    /// Migrates from the old Tauri store location on first access.
    pub fn open(path: PathBuf) -> Self {
        let data = Self::load_from_disk(&path).unwrap_or_default();
        Self {
            path,
            data: Mutex::new(data),
        }
    }

    /// Convenience: opens from `$XDG_CONFIG_HOME/meeting-assistant/settings.json`
    /// (or `~/.config/meeting-assistant/settings.json` on Linux).
    pub fn open_default() -> Self {
        let path = config_dir().join("meeting-assistant/settings.json");
        let mut store = Self::open(path);
        store.try_migrate_tauri_store();
        store
    }

    pub fn load(&self) -> PersistedSettings {
        self.data.lock().unwrap().clone()
    }

    pub fn save(&self, settings: PersistedSettings) -> std::io::Result<()> {
        let settings = settings.normalize();
        *self.data.lock().unwrap() = settings.clone();
        self.flush_to_disk(&settings)
    }

    // ── internals ─────────────────────────────────────────────────────────────

    fn load_from_disk(path: &PathBuf) -> Option<PersistedSettings> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str::<PersistedSettings>(&content)
            .ok()
            .map(PersistedSettings::normalize)
    }

    fn flush_to_disk(&self, settings: &PersistedSettings) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&settings.clone().normalize())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.path, json)
    }

    /// One-time migration: read the old Tauri plugin-store JSON and import settings.
    fn try_migrate_tauri_store(&mut self) {
        // Only migrate if our own file doesn't exist yet.
        if self.path.exists() {
            return;
        }
        let old_path = xdg_data_dir().join("dev.codemedvedev.meeting-assistant/settings.json");
        let content = match std::fs::read_to_string(&old_path) {
            Ok(c) => c,
            Err(_) => return,
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return,
        };
        let str_val = |key: &str| -> Option<String> {
            json.get(key)?
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let mut settings = PersistedSettings::default();
        settings.paths.model = str_val("paths.model");
        settings.paths.models_dir = str_val("paths.models_dir");
        settings.paths.db = str_val("paths.db");
        settings.paths.meetings_dir = str_val("paths.recordings");
        settings.paths.prompts = str_val("paths.prompts");
        settings.anthropic_api_key = str_val("anthropic_api_key");
        settings.recording.source = str_val("recording.source").unwrap_or_else(|| "mic".into());
        settings.recording.echo_cancel = json
            .get("recording.echo_cancel")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        settings.default_template = str_val("default_template");
        // Persist to new location (ignore error; migration is best-effort).
        let settings = settings.normalize();
        let _ = self.flush_to_disk(&settings);
        *self.data.lock().unwrap() = settings;
    }
}

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}

fn xdg_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        })
}

fn default_models_dir() -> PathBuf {
    xdg_data_dir().join("meeting-assistant/models")
}

#[cfg(test)]
mod tests {
    use super::{PersistedSettings, PersistedTranscriberPrefs, TranscriptionModelSource};

    #[test]
    fn transcriber_model_path_empty_string_deserializes_as_none() {
        let prefs: PersistedTranscriberPrefs = serde_json::from_str(
            r#"{
                "language": "ru",
                "beam_size": 1,
                "n_threads": 0,
                "model_path": ""
            }"#,
        )
        .expect("transcriber prefs JSON should parse");

        assert_eq!(prefs.custom_model_path, None);
    }

    #[test]
    fn transcriber_model_path_trims_non_empty_value() {
        let prefs: PersistedTranscriberPrefs = serde_json::from_str(
            r#"{
                "language": "ru",
                "beam_size": 1,
                "n_threads": 0,
                "model_path": "  /models/ggml-medium.bin  "
            }"#,
        )
        .expect("transcriber prefs JSON should parse");

        assert_eq!(
            prefs.custom_model_path.as_deref(),
            Some("/models/ggml-medium.bin")
        );
    }

    #[test]
    fn legacy_transcriber_model_path_migrates_to_custom_path() {
        let settings: PersistedSettings = serde_json::from_str(
            r#"{
                "paths": {},
                "recording": { "source": "mic", "echo_cancel": false },
                "transcriber": {
                    "language": "ru",
                    "beam_size": 1,
                    "n_threads": 0,
                    "model_path": "/models/ggml-medium.bin"
                }
            }"#,
        )
        .expect("settings JSON should parse");

        let settings = settings.normalize();

        assert_eq!(
            settings.transcriber.model_source,
            TranscriptionModelSource::CustomPath
        );
        assert_eq!(
            settings.transcriber.custom_model_path.as_deref(),
            Some("/models/ggml-medium.bin")
        );
    }

    #[test]
    fn legacy_paths_model_migrates_when_transcriber_path_is_absent() {
        let settings: PersistedSettings = serde_json::from_str(
            r#"{
                "paths": { "model": "/legacy/ggml-base.bin" },
                "recording": { "source": "mic", "echo_cancel": false }
            }"#,
        )
        .expect("settings JSON should parse");

        let settings = settings.normalize();

        assert_eq!(
            settings.transcriber.model_source,
            TranscriptionModelSource::CustomPath
        );
        assert_eq!(
            settings.transcriber.custom_model_path.as_deref(),
            Some("/legacy/ggml-base.bin")
        );
    }

    #[test]
    fn custom_model_path_is_added_to_custom_models_list() {
        let settings: PersistedSettings = serde_json::from_str(
            r#"{
                "paths": {},
                "recording": { "source": "mic", "echo_cancel": false },
                "transcriber": {
                    "language": "ru",
                    "beam_size": 1,
                    "n_threads": 0,
                    "custom_model_path": "/models/ggml-large-v3-turbo-q5_0.bin",
                    "custom_models": []
                }
            }"#,
        )
        .expect("settings JSON should parse");

        let settings = settings.normalize();

        assert_eq!(settings.transcriber.custom_models.len(), 1);
        assert_eq!(
            settings.transcriber.custom_models[0].path,
            "/models/ggml-large-v3-turbo-q5_0.bin"
        );
        assert_eq!(
            settings.transcriber.custom_models[0].name,
            "large-v3-turbo-q5_0"
        );
    }
}
