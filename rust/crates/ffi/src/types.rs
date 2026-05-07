/// All data types exchanged between Kotlin and Rust through the UniFFI boundary.
/// Every public struct here must derive uniffi::Record (maps to a Kotlin data class).
/// Every public enum used as error must derive uniffi::Error.

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AppError {
    #[error("{0}")]
    General(String),
}

impl AppError {
    pub fn general(e: impl std::fmt::Display) -> Self {
        AppError::General(e.to_string())
    }
}

pub type FfiResult<T> = Result<T, AppError>;

// ── AppConfig ─────────────────────────────────────────────────────────────────

/// Configuration passed to `init_core`. All fields are optional — missing values
/// fall back to `settings.json` on disk, then environment variables, then defaults.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AppConfig {
    pub model_path:     Option<String>,
    pub db_path:        Option<String>,
    pub recordings_dir: Option<String>,
    pub prompts_dir:    Option<String>,
    /// If `Some`, overrides the stored API key for this session (written to neither disk nor env
    /// by `init_core` itself — caller decides). Pass `None` to use the stored/env key.
    pub anthropic_api_key: Option<String>,
}

// ── Meeting / Recording / Protocol ────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct MeetingDto {
    pub id: String,
    pub name: String,
    pub audio_path: String,
    pub has_transcript: bool,
    pub has_protocol: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct RecordingDto {
    pub id: String,
    pub name: String,
    pub audio_path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct ProtocolDto {
    pub markdown: String,
}

// ── Job ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct JobDto {
    pub id: String,
    pub meeting_id: String,
    pub status: String,
    pub attempts: u32,
    pub last_error: Option<String>,
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct SettingsPathsDto {
    pub model: Option<String>,
    pub db: Option<String>,
    pub recordings: Option<String>,
    pub prompts: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct RecordingPrefsDto {
    pub source: String,
    pub echo_cancel: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct SettingsDto {
    pub paths: SettingsPathsDto,
    /// None = key not exposed (settings_get never returns it).
    /// Some("") = clear key.
    /// Some("sk-...") = set new key.
    pub anthropic_api_key: Option<String>,
    pub recording: RecordingPrefsDto,
    pub default_template: Option<String>,
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct PathInfoDto {
    pub path: String,
    pub exists: bool,
    pub writable: bool,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct DeviceInfoDto {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct DiagnosticsPathsDto {
    pub model: PathInfoDto,
    pub db: PathInfoDto,
    pub recordings: PathInfoDto,
    pub prompts: PathInfoDto,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct DiagnosticsDto {
    pub os: String,
    pub arch: String,
    pub app_version: String,
    pub cpal_host: String,
    pub input_devices: Vec<DeviceInfoDto>,
    pub output_devices: Vec<DeviceInfoDto>,
    pub paths: DiagnosticsPathsDto,
    pub has_anthropic_key: bool,
    pub ffmpeg_ok: bool,
    pub logs: Vec<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn job_to_dto(j: meeting_core::entities::Job) -> JobDto {
    JobDto {
        id:         j.id,
        meeting_id: j.meeting_id,
        status:     j.status.as_str().to_string(),
        attempts:   j.attempts,
        last_error: j.last_error,
    }
}

pub(crate) fn path_info(p: &std::path::Path) -> PathInfoDto {
    let meta = std::fs::metadata(p);
    let exists = meta.is_ok();
    let size_bytes = meta.as_ref().ok().map(|m| m.len());
    let writable = if p.is_file() {
        std::fs::OpenOptions::new().write(true).open(p).is_ok()
    } else {
        let dir = if p.is_dir() { p } else { p.parent().unwrap_or(p) };
        let test = dir.join(".writable_test");
        let ok = std::fs::write(&test, b"").is_ok();
        let _ = std::fs::remove_file(&test);
        ok
    };
    PathInfoDto { path: p.display().to_string(), exists, writable, size_bytes }
}
