use std::path::Path;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::CoreError;

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

#[async_trait]
pub trait AudioCapture: Send + Sync {
    /// Begin capturing audio from `source`, writing PCM to `output_path`.
    async fn start_session(
        &self,
        session_id: &str,
        output_path: &Path,
        source: CaptureSource,
        echo_cancel: bool,
    ) -> Result<(), CoreError>;

    /// Stop the session, finalize the output file, and wait for the writer to flush.
    async fn stop_session(&self, session_id: &str) -> Result<(), CoreError>;

    fn is_active(&self, session_id: &str) -> bool;
}
