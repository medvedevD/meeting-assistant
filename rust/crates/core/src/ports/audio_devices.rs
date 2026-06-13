use crate::CoreError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One selectable audio device.
///
/// `id` is the platform-native name used to pin a selection across the API
/// boundary and to match the device again at capture time (cpal `Device::name()`,
/// a PulseAudio source name, or a WASAPI device name). `label` is the
/// human-friendly text shown in the UI — usually identical to `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

/// The devices available to capture from, split by leg.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDeviceList {
    /// Microphone inputs (all platforms).
    pub input: Vec<AudioDevice>,
    /// System-audio (loopback) sources. Empty on macOS.
    pub output: Vec<AudioDevice>,
    /// Whether a specific system-audio device can be chosen. `false` on macOS,
    /// where ScreenCaptureKit captures the aggregate system mix with no
    /// per-output handle — the UI hides the system picker accordingly.
    pub system_selectable: bool,
}

/// Read-only enumeration of capture devices. Kept separate from
/// [`super::AudioCapture`] (ISP): the list path is pure and trivially fakeable.
#[async_trait]
pub trait AudioDeviceEnumerator: Send + Sync {
    async fn list_devices(&self) -> Result<AudioDeviceList, CoreError>;
}
