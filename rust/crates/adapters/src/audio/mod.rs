mod cpal_capture;
pub use cpal_capture::CpalAudioCapture;

/// Crash-safe recording recovery — startup pass that finalizes orphaned WAVs
/// and reconciles missing DB rows (section-06).
pub mod recovery;
pub use recovery::{run_recovery, RecoveryReport, WavRecovery};

/// macOS system-audio backend (ScreenCaptureKit, audio-only). See section-05.
#[cfg(target_os = "macos")]
mod sck_capture;
