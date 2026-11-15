use serde::Serialize;

use crate::InstallationView;

/// Payload of `GET /api/v1/setup-status`: whether the app can do its job, and
/// what is missing if it cannot. The first-run steps, the recording screen and
/// the launch notice all read this one answer.
#[derive(Debug, Clone, Serialize)]
pub struct SetupStatus {
    pub transcription: TranscriptionSetup,
    pub minutes: MinutesSetup,
    /// `false` when this CPU lacks the instructions the Whisper build needs, so
    /// a transcription would kill the sidecar.
    pub cpu_supported: bool,
    /// Recordings the startup recovery pass repaired in this run.
    pub recovered_recordings: Vec<RecoveredRecording>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionSetupState {
    /// The selected model file is in place.
    Ready,
    /// No usable model yet, and one is being downloaded.
    Downloading,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionSetup {
    pub state: TranscriptionSetupState,
    /// Why there is no usable model: `model_not_selected` (never set up) or
    /// `model_missing` (a model was chosen and its file is gone). `None` when
    /// ready.
    pub reason: Option<String>,
    /// `managed` or `custom_path`.
    pub source: String,
    /// The selected catalogue model, if the source is `managed`.
    pub model_id: Option<String>,
    /// The catalogue model to offer a user who has none.
    pub recommended_model_id: String,
    /// The download in progress, if any.
    pub installation: Option<InstallationView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MinutesSetupState {
    /// The active provider has what it needs to be called. This is not a
    /// promise that the key is valid or the provider reachable.
    Ready,
    Missing,
    /// No key, and the user said they will set minutes up later.
    Skipped,
    /// The passphrase vault is locked, so whether a key exists is unknown.
    Locked,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinutesSetup {
    pub state: MinutesSetupState,
    /// The active provider's id.
    pub provider: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveredRecording {
    pub meeting_id: String,
    pub name: String,
}

/// Answers `GET /api/v1/setup-status`. The GUI asks at every launch, before the
/// user has done anything, so an implementation must not read a key from the OS
/// keyring or open a capture device: on macOS both can raise a system dialog.
pub trait SetupStatusService: Send + Sync {
    fn status(&self) -> SetupStatus;
}
