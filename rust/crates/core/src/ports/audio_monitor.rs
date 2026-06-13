use crate::ports::{CaptureSpec, ResolvedDevices};
use crate::CoreError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A live input-level sample for the device-test meter.
///
/// `level` is a linear 0.0–1.0 peak (ready for a progress bar); `peak_db` is the
/// same value in dBFS (≤ 0), clamped at a `-60 dB` noise floor for display.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioLevel {
    pub level: f32,
    pub peak_db: f32,
}

/// Captures a single device (mic **or** system) without writing a file, exposing
/// a continuously-updated input level so the settings UI can show a live meter.
///
/// Only one leg is monitored per session — the test never mixes. Sessions are
/// keyed by an opaque `id` the caller mints; `start` returns the resolved device
/// label so the UI shows exactly what is being metered.
#[async_trait]
pub trait AudioLevelMonitor: Send + Sync {
    async fn start(&self, id: &str, spec: CaptureSpec) -> Result<ResolvedDevices, CoreError>;

    /// The latest level for `id`, or `None` if no such session is active.
    fn level(&self, id: &str) -> Option<AudioLevel>;

    async fn stop(&self, id: &str) -> Result<(), CoreError>;
}
