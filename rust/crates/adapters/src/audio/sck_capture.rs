//! macOS system-audio capture via ScreenCaptureKit (**audio-only**).
//!
//! QtMultimedia/cpal cannot expose a macOS loopback device, so on macOS the
//! `System` / `Mixed` capture sources go through ScreenCaptureKit. macOS 13.0
//! is the floor (`SCStreamConfiguration.capturesAudio`); the
//! `screencapturekit` crate is pinned because its API churns rapidly.
//!
//! ## Privacy / scope guarantees (acceptance criterion: no screen frames)
//!
//! This module captures audio and **only** audio:
//!
//! * the stream is created with a single output handler bound to
//!   [`SCStreamOutputType::Audio`] — a `Screen` handler is **never** added, so
//!   ScreenCaptureKit never delivers a video `CMSampleBuffer` to us;
//! * we never call `image_buffer()` / `IOSurface` / any pixel API on a sample;
//! * the content filter references a display only because ScreenCaptureKit
//!   requires a content source to exist — we request no frames from it.
//!
//! ## Footguns (documented in code, per section-05)
//!
//! * **Frame-drop:** with audio enabled but no screen output, ScreenCaptureKit
//!   will drop audio frames unless the (unused) video path is throttled. We do
//!   *not* add a real screen output; instead we set an enormous
//!   `minimumFrameInterval` (one "frame" per 24 h) so the video path stays
//!   idle while audio flows.
//! * **Lifetime:** the `SCStream` owns the audio handler; if either is dropped
//!   the callback is freed and audio silently stops. Both are kept alive for
//!   the whole session and only dropped after `stop_capture()`.
//!
//! ## TCC re-prompt UX
//!
//! ScreenCaptureKit gates on the **Screen Recording** TCC right (there is no
//! audio-only prompt). v1 is ad-hoc/self-signed, so the grant resets every
//! time the signing-identity hash changes — i.e. on every app update — and is
//! also absent on first run. Both cases are surfaced identically via
//! [`permission_guidance`], which includes a deep link to the exact Settings
//! pane. Permission failures are detected both at a cheap pre-flight
//! ([`preflight`]) and from `start_capture`.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use hound::{SampleFormat, WavSpec, WavWriter};
use screencapturekit::error::{SCError, SCStreamErrorCode};
use screencapturekit::prelude::*;
use screencapturekit::AudioBufferList;

/// Deep link to System Settings → Privacy & Security → Screen Recording.
/// Stable across modern macOS (Ventura 13 … Sequoia 15 …).
pub const SCREEN_RECORDING_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

/// WAV spec kept identical to the Linux/Windows backends: f32 / 48 kHz / 2-ch.
const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

type Sink = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

/// Actionable, user-facing guidance for a missing/declined Screen-Recording
/// grant. Intentionally identical for first-run and post-update: from the
/// user's point of view the action is the same (re-enable in Settings), and
/// the ad-hoc-signing reset makes the two indistinguishable anyway.
pub fn permission_guidance() -> String {
    format!(
        "Screen Recording permission is required to capture system audio on macOS. \
         macOS labels this permission \"Screen Recording\", but Meeting Assistant \
         records audio only and never captures the screen. \
         Open System Settings → Privacy & Security → Screen Recording, enable \
         Meeting Assistant, then start recording again. \
         If you just installed or updated the app you must (re-)enable it there — \
         the grant resets when the app is updated. \
         Open the settings pane directly: {SCREEN_RECORDING_SETTINGS_URL}"
    )
}

/// True if `err` indicates the Screen-Recording TCC right is missing/declined.
fn is_permission_error(err: &SCError) -> bool {
    matches!(
        err.stream_error_code(),
        Some(SCStreamErrorCode::UserDeclined | SCStreamErrorCode::MissingEntitlements)
    ) || matches!(err, SCError::PermissionDenied(_))
}

/// Map an `SCError` to a user-facing string: guided message for permission
/// problems, otherwise the underlying error (still actionable, no crash).
fn map_err(err: SCError) -> String {
    if is_permission_error(&err) {
        permission_guidance()
    } else {
        format!("ScreenCaptureKit error: {err}. {}", permission_guidance())
    }
}

/// Cheap pre-flight so a denied permission surfaces at *start* time (good UX)
/// instead of only when the recording is stopped. Called from
/// `start_session` before the capture thread is spawned.
///
/// `SCShareableContent::get()` is the canonical permission probe: it fails (or
/// returns no displays) when Screen Recording is not granted.
pub fn preflight() -> Result<(), String> {
    match SCShareableContent::get() {
        Ok(content) if !content.displays().is_empty() => Ok(()),
        Ok(_) => Err(permission_guidance()),
        Err(e) => Err(map_err(e)),
    }
}

/// Audio-only stream output. Receives system-audio `CMSampleBuffer`s and
/// appends them to the WAV writer. Screen buffers are ignored defensively,
/// though none are ever requested (no `Screen` handler is registered).
struct AudioSink {
    writer: Sink,
}

impl SCStreamOutputTrait for AudioSink {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        // Audio only. We never inspect or retain screen frames.
        if of_type != SCStreamOutputType::Audio {
            return;
        }
        let Some(list) = sample.audio_buffer_list() else {
            return;
        };
        let mut guard = match self.writer.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(w) = guard.as_mut() {
            write_buffer_list(w, &list);
        }
    }
}

/// Reinterpret a byte slice of native-endian `f32` PCM as `f32` samples.
/// Apple platforms are little-endian; ScreenCaptureKit delivers 32-bit float.
fn bytes_as_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(f32::from_le_bytes)
        .collect()
}

/// Write one ScreenCaptureKit audio sample (an `AudioBufferList`) into the
/// WAV writer as interleaved stereo f32.
///
/// ScreenCaptureKit normally delivers **non-interleaved (planar)** float PCM:
/// one `AudioBuffer` per channel. We also handle the interleaved single-buffer
/// shape, and up-mix mono → stereo, so the on-disk WAV always matches the
/// 2-channel spec the rest of the pipeline expects.
fn write_buffer_list(w: &mut WavWriter<BufWriter<File>>, list: &AudioBufferList) {
    let n = list.num_buffers();
    if n == 0 {
        return;
    }

    if n == 1 {
        let Some(buf) = list.get(0) else { return };
        let samples = bytes_as_f32(buf.data());
        let ch = buf.number_channels.max(1) as usize;
        if ch >= CHANNELS as usize {
            // Interleaved with ≥2 channels: take the first two.
            for frame in samples.chunks_exact(ch) {
                let _ = w.write_sample(frame[0]);
                let _ = w.write_sample(frame[1]);
            }
        } else {
            // Mono: duplicate into both channels.
            for s in samples {
                let _ = w.write_sample(s);
                let _ = w.write_sample(s);
            }
        }
        return;
    }

    // Planar: buffer 0 = left, buffer 1 = right (further buffers ignored —
    // the output spec is stereo).
    let left = list
        .get(0)
        .map(|b| bytes_as_f32(b.data()))
        .unwrap_or_default();
    let right = list
        .get(1)
        .map(|b| bytes_as_f32(b.data()))
        .unwrap_or_default();
    let frames = left.len().min(right.len());
    for i in 0..frames {
        let _ = w.write_sample(left[i]);
        let _ = w.write_sample(right[i]);
    }
}

/// Capture macOS **system** audio via ScreenCaptureKit to `output_path` until
/// a value is received on `stop_rx`. Writes f32 / 48 kHz / 2-ch WAV, matching
/// the Linux (`parec`) and Windows (WASAPI) backends.
pub fn record_system(output_path: PathBuf, stop_rx: Receiver<()>) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let content = SCShareableContent::get().map_err(map_err)?;
    let displays = content.displays();
    let display = displays.first().ok_or_else(permission_guidance)?;

    // The filter must reference a content source for ScreenCaptureKit to
    // start, but we register no screen output, so no frames are captured.
    let filter = SCContentFilter::create()
        .with_display(display)
        .with_excluding_windows(&[])
        .build();

    // Footgun mitigation: one nominal "frame" per 24 h keeps the unused video
    // path idle so audio frames are not dropped (no screen output is added).
    // A tiny size further minimises any video-path allocation.
    let huge_interval = CMTime::new(86_400, 1);
    let config = SCStreamConfiguration::new()
        .with_captures_audio(true)
        .with_sample_rate(SAMPLE_RATE as i32)
        .with_channel_count(i32::from(CHANNELS))
        .with_width(2)
        .with_height(2)
        .with_minimum_frame_interval(&huge_interval);

    let spec = WavSpec {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let file = File::create(&output_path).map_err(|e| e.to_string())?;
    let writer: Sink = Arc::new(Mutex::new(Some(
        WavWriter::new(BufWriter::new(file), spec).map_err(|e| e.to_string())?,
    )));

    let mut stream = SCStream::new(&filter, &config);
    // ONLY an audio handler — never a Screen handler. This is the structural
    // guarantee that no screen frames are ever requested or delivered.
    stream.add_output_handler(
        AudioSink {
            writer: Arc::clone(&writer),
        },
        SCStreamOutputType::Audio,
    );

    stream.start_capture().map_err(map_err)?;

    // Keep `stream` (hence the audio handler) alive for the whole session.
    let _ = stop_rx.recv();

    stream.stop_capture().map_err(|e| e.to_string())?;
    drop(stream); // release ScreenCaptureKit before finalising the WAV

    let w = writer
        .lock()
        .map_err(|_| "audio writer mutex poisoned".to_string())?
        .take()
        .ok_or_else(|| "audio writer already finalised".to_string())?;
    w.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_mentions_screen_recording_and_deep_link() {
        let g = permission_guidance();
        assert!(g.contains("Screen Recording"), "missing TCC name: {g}");
        assert!(g.contains("audio only"), "should clarify scope: {g}");
        assert!(
            g.contains(SCREEN_RECORDING_SETTINGS_URL),
            "missing deep link: {g}"
        );
    }

    #[test]
    fn permission_error_codes_are_classified() {
        let declined = SCError::from_stream_error_code(SCStreamErrorCode::UserDeclined);
        let missing = SCError::from_stream_error_code(SCStreamErrorCode::MissingEntitlements);
        assert!(is_permission_error(&declined));
        assert!(is_permission_error(&missing));
        // Map collapses permission failures to the guided message.
        assert_eq!(map_err(declined), permission_guidance());
    }

    #[test]
    fn non_permission_error_is_actionable_not_swallowed() {
        let other = SCError::stream_error("display list unavailable");
        assert!(!is_permission_error(&other));
        let msg = map_err(other);
        assert!(msg.contains("display list unavailable"), "{msg}");
        // Still points the user at the likely fix.
        assert!(msg.contains(SCREEN_RECORDING_SETTINGS_URL), "{msg}");
    }

    #[test]
    fn bytes_as_f32_round_trips_le() {
        let mut bytes = Vec::new();
        for s in [0.0f32, 1.0, -0.5, 0.25] {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        assert_eq!(bytes_as_f32(&bytes), vec![0.0, 1.0, -0.5, 0.25]);
    }

    #[test]
    fn bytes_as_f32_ignores_trailing_partial_sample() {
        let bytes = [0, 0, 0, 0, 1, 2]; // 1 full f32 + 2 stray bytes
        assert_eq!(bytes_as_f32(&bytes), vec![0.0]);
    }
}
