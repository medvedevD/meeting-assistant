use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Persisted user settings. All fields are optional to allow partial saves and forward-compat.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedSettings {
    pub paths: PersistedPaths,
    /// Stored in plaintext in the config file. On first run migrated from the old Tauri store.
    pub anthropic_api_key: Option<String>,
    pub recording: RecordingPrefs,
    pub default_template: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedPaths {
    pub model: Option<String>,
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
        Self { source: "mic".into(), echo_cancel: false }
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
        Self { path, data: Mutex::new(data) }
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
        *self.data.lock().unwrap() = settings.clone();
        self.flush_to_disk(&settings)
    }

    // ── internals ─────────────────────────────────────────────────────────────

    fn load_from_disk(path: &PathBuf) -> Option<PersistedSettings> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn flush_to_disk(&self, settings: &PersistedSettings) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.path, json)
    }

    /// One-time migration: read the old Tauri plugin-store JSON and import settings.
    fn try_migrate_tauri_store(&mut self) {
        // Only migrate if our own file doesn't exist yet.
        if self.path.exists() {
            return;
        }
        let old_path = xdg_data_dir()
            .join("dev.codemedvedev.meeting-assistant/settings.json");
        let content = match std::fs::read_to_string(&old_path) {
            Ok(c) => c,
            Err(_) => return,
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return,
        };
        let str_val = |key: &str| -> Option<String> {
            json.get(key)?.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
        };
        let mut settings = PersistedSettings::default();
        settings.paths.model        = str_val("paths.model");
        settings.paths.db           = str_val("paths.db");
        settings.paths.meetings_dir = str_val("paths.recordings");
        settings.paths.prompts      = str_val("paths.prompts");
        settings.anthropic_api_key = str_val("anthropic_api_key");
        settings.recording.source =
            str_val("recording.source").unwrap_or_else(|| "mic".into());
        settings.recording.echo_cancel =
            json.get("recording.echo_cancel")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        settings.default_template = str_val("default_template");
        // Persist to new location (ignore error; migration is best-effort).
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
