use crate::CoreError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Which audio source(s) to capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    /// Default microphone input only.
    #[default]
    Mic,
    /// System/loopback audio only (PipeWire monitor on Linux, WASAPI loopback on Windows).
    System,
    /// Mic + system audio mixed into one file.
    Mixed,
}

/// A fully-specified capture request.
///
/// `mic_device` / `system_device` are the platform-native device *names*
/// (cpal `Device::name()`, a PulseAudio source name, or a WASAPI device name).
/// `None` means "follow the OS default" — the sticky, sane default that keeps
/// working when devices are plugged/unplugged. A device named here that has
/// since vanished is resolved back to the OS default (see [`ResolvedDevices`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CaptureSpec {
    pub source: CaptureSource,
    pub echo_cancel: bool,
    #[serde(default)]
    pub mic_device: Option<String>,
    /// Ignored on macOS, where ScreenCaptureKit captures the aggregate system mix.
    #[serde(default)]
    pub system_device: Option<String>,
}

/// The device labels actually opened for a session — what the UI shows so the
/// user can *see* which source is live, including any default fallback.
///
/// `None` means that leg was not part of the source (e.g. no system leg for a
/// mic-only recording).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedDevices {
    pub mic: Option<String>,
    pub system: Option<String>,
}

#[async_trait]
pub trait AudioCapture: Send + Sync {
    /// Begin capturing audio for `spec`, writing PCM to `output_path`.
    ///
    /// Devices are resolved *before* capture starts, so a missing device or a
    /// denied permission surfaces here rather than only on stop. Returns the
    /// labels actually opened.
    async fn start_session(
        &self,
        session_id: &str,
        output_path: &Path,
        spec: CaptureSpec,
    ) -> Result<ResolvedDevices, CoreError>;

    /// Stop the session, finalize the output file, and wait for the writer to flush.
    async fn stop_session(&self, session_id: &str) -> Result<(), CoreError>;

    fn is_active(&self, session_id: &str) -> bool;
}
