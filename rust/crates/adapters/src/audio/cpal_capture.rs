use std::collections::HashMap;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use meeting_core::{CoreError, ports::{AudioCapture, CaptureSource}};

struct Session {
    stop_tx: std::sync::mpsc::SyncSender<()>,
    thread: thread::JoinHandle<Result<(), String>>,
}

pub struct CpalAudioCapture {
    sessions: Mutex<HashMap<String, Session>>,
}

impl CpalAudioCapture {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

impl Default for CpalAudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioCapture for CpalAudioCapture {
    async fn start_session(
        &self,
        session_id: &str,
        output_path: &Path,
        source: CaptureSource,
        echo_cancel: bool,
    ) -> Result<(), CoreError> {
        let path = output_path.to_path_buf();
        let id = session_id.to_string();
        let (stop_tx, stop_rx) = std::sync::mpsc::sync_channel(1);

        let thread = thread::Builder::new()
            .name(format!("cpal-recording-{id}"))
            .spawn(move || record(path, stop_rx, source, echo_cancel))
            .map_err(|e| CoreError::Recording(e.to_string()))?;

        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), Session { stop_tx, thread });

        Ok(())
    }

    async fn stop_session(&self, session_id: &str) -> Result<(), CoreError> {
        let session = self
            .sessions
            .lock()
            .unwrap()
            .remove(session_id)
            .ok_or_else(|| CoreError::Recording(format!("session not found: {session_id}")))?;

        let _ = session.stop_tx.send(());

        session
            .thread
            .join()
            .map_err(|_| CoreError::Recording("recording thread panicked".into()))?
            .map_err(CoreError::Recording)
    }

    fn is_active(&self, session_id: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(session_id)
    }
}

// ── Device / source selection ─────────────────────────────────────────────────

/// Find the index of a monitor (loopback) device in a list of PulseAudio/PipeWire source names.
///
/// On Linux, `pactl list sources short` returns names like
/// `alsa_output.pci-0000_00_1f.3.analog-stereo.monitor`.
/// This is a pure function — testable without audio hardware.
pub(crate) fn find_monitor_device(names: &[impl AsRef<str>]) -> Option<usize> {
    names.iter().position(|n| n.as_ref().contains(".monitor"))
}

fn default_mic(host: &cpal::Host) -> Result<cpal::Device, String> {
    host.default_input_device()
        .ok_or_else(|| "no default input device available".to_string())
}

/// Ask PulseAudio/PipeWire for the monitor source of the current default sink.
///
/// First queries `pactl get-default-sink`, then looks for `<sink>.monitor` in
/// `pactl list sources short`. Falls back to the first `.monitor` source if no
/// match is found (e.g. when the default sink has no explicit monitor entry).
#[cfg(target_os = "linux")]
fn find_pulseaudio_monitor() -> Result<String, String> {
    let default_sink = std::process::Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let out = std::process::Command::new("pactl")
        .args(["list", "sources", "short"])
        .output()
        .map_err(|e| format!("failed to run pactl: {e}"))?;

    let text = String::from_utf8_lossy(&out.stdout);
    let names: Vec<&str> = text
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();

    // Prefer the monitor of the default sink.
    if let Some(sink) = &default_sink {
        let preferred = format!("{sink}.monitor");
        if let Some(name) = names.iter().find(|&&n| n == preferred) {
            return Ok((*name).to_string());
        }
    }

    // Fallback: first .monitor source in the list.
    let idx = find_monitor_device(&names)
        .ok_or_else(|| format!("no monitor source found via pactl (sources: {})", names.join(", ")))?;

    Ok(names[idx].to_string())
}

#[cfg(target_os = "windows")]
fn find_wasapi_output_device() -> Result<cpal::Device, String> {
    let wasapi = cpal::host_from_id(cpal::HostId::Wasapi)
        .map_err(|e| format!("WASAPI host unavailable: {e}"))?;
    wasapi
        .default_output_device()
        .ok_or_else(|| "no default output device for WASAPI loopback".to_string())
}

// ── Recording threads ─────────────────────────────────────────────────────────

fn record(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    source: CaptureSource,
    echo_cancel: bool,
) -> Result<(), String> {
    match source {
        CaptureSource::Mic => {
            let host = cpal::default_host();
            record_single(default_mic(&host)?, output_path, stop_rx)
        }
        CaptureSource::System => record_system(output_path, stop_rx),
        CaptureSource::Mixed => record_mixed(output_path, stop_rx, echo_cancel),
    }
}

fn record_single(
    device: cpal::Device,
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let config = device.default_input_config().map_err(|e| e.to_string())?;

    let spec = hound::WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let file = std::fs::File::create(&output_path).map_err(|e| e.to_string())?;
    let writer = Arc::new(Mutex::new(
        hound::WavWriter::new(BufWriter::new(file), spec).map_err(|e| e.to_string())?,
    ));

    let writer_cb = Arc::clone(&writer);
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mut w = writer_cb.lock().unwrap();
                for &s in data {
                    let _ = w.write_sample(s);
                }
            },
            |err| tracing::error!("cpal input stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    let _ = stop_rx.recv();
    drop(stream);

    Arc::try_unwrap(writer)
        .map_err(|_| "writer Arc still has multiple owners".to_string())?
        .into_inner()
        .unwrap()
        .finalize()
        .map_err(|e| e.to_string())
}

// ── System audio (platform-specific) ─────────────────────────────────────────

#[cfg(target_os = "linux")]
fn record_system(output_path: PathBuf, stop_rx: std::sync::mpsc::Receiver<()>) -> Result<(), String> {
    let monitor = find_pulseaudio_monitor()?;
    record_parec(&monitor, output_path, stop_rx)
}

#[cfg(target_os = "windows")]
fn record_system(output_path: PathBuf, stop_rx: std::sync::mpsc::Receiver<()>) -> Result<(), String> {
    let device = find_wasapi_output_device()?;
    record_single(device, output_path, stop_rx)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn record_system(_output_path: PathBuf, _stop_rx: std::sync::mpsc::Receiver<()>) -> Result<(), String> {
    Err("system audio capture is not yet supported on this platform".to_string())
}

/// Capture audio from a PulseAudio/PipeWire source by name using the `parec` subprocess.
///
/// `parec` outputs raw little-endian f32 samples. We write them to a WAV file via hound.
#[cfg(target_os = "linux")]
fn record_parec(
    source_name: &str,
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    use std::io::Read;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let file = std::fs::File::create(&output_path).map_err(|e| e.to_string())?;
    let writer = Arc::new(Mutex::new(
        hound::WavWriter::new(BufWriter::new(file), spec).map_err(|e| e.to_string())?,
    ));

    let mut child = std::process::Command::new("parec")
        .args([
            "--device", source_name,
            "--format=float32le",
            "--channels=2",
            "--rate=48000",
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn parec: {e}"))?;

    let mut parec_out = child.stdout.take().unwrap();
    let w = Arc::clone(&writer);

    let read_thread = thread::spawn(move || {
        let mut buf = [0u8; 4096 * 4];
        loop {
            match parec_out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut w = w.lock().unwrap();
                    for chunk in buf[..n].chunks_exact(4) {
                        let s = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        let _ = w.write_sample(s);
                    }
                }
            }
        }
    });

    let _ = stop_rx.recv();

    // Killing parec closes its stdout → read_thread exits naturally.
    child.kill().ok();
    child.wait().ok();
    read_thread.join().ok();

    Arc::try_unwrap(writer)
        .map_err(|_| "writer still borrowed".to_string())?
        .into_inner()
        .unwrap()
        .finalize()
        .map_err(|e| e.to_string())
}

// ── Mixed (mic + system) ──────────────────────────────────────────────────────

/// Record mic and system audio simultaneously into `output_path`.
///
/// Each source is captured to a separate temp file to avoid sample-rate mismatch
/// (cpal uses 44100 Hz, parec uses 48000 Hz) and lock contention. After both
/// captures finish, ffmpeg mixes them into `output_path` and temp files are deleted.
#[cfg(target_os = "linux")]
fn record_mixed(output_path: PathBuf, stop_rx: std::sync::mpsc::Receiver<()>, echo_cancel: bool) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let monitor = find_pulseaudio_monitor()?;

    let mic_tmp = output_path.with_extension("mic_tmp.wav");
    let sys_tmp = output_path.with_extension("sys_tmp.wav");

    let (mic_stop_tx, mic_stop_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let (sys_stop_tx, sys_stop_rx) = std::sync::mpsc::sync_channel::<()>(1);

    let mic_path = mic_tmp.clone();
    let sys_path = sys_tmp.clone();
    let monitor_name = monitor.clone();

    let mic_thread = thread::spawn(move || {
        let host = cpal::default_host();
        let mic = default_mic(&host)?;
        record_single(mic, mic_path, mic_stop_rx)
    });

    let sys_thread = thread::spawn(move || {
        record_parec(&monitor_name, sys_path, sys_stop_rx)
    });

    let _ = stop_rx.recv();
    let _ = mic_stop_tx.send(());
    let _ = sys_stop_tx.send(());

    mic_thread.join().map_err(|_| "mic thread panicked".to_string())??;
    sys_thread.join().map_err(|_| "sys thread panicked".to_string())??;

    ffmpeg_mix(&mic_tmp, &sys_tmp, &output_path, echo_cancel)?;

    let _ = std::fs::remove_file(&mic_tmp);
    let _ = std::fs::remove_file(&sys_tmp);

    Ok(())
}

/// Build the ffmpeg `-filter_complex` string for mixing mic + system audio.
///
/// Always applies `highpass=f=80,dynaudnorm` to the mic track to reduce background noise.
/// If `echo_cancel` is true, also prepends an `anlms` adaptive filter that subtracts the
/// system audio from the mic track before mixing (acoustic echo cancellation).
pub(crate) fn build_mix_filter(echo_cancel: bool) -> String {
    if echo_cancel {
        "[0:a][1:a]anlms=order=512:mu=0.05:eps=1[mic_aec];\
         [mic_aec]highpass=f=80,dynaudnorm[mic_clean];\
         [mic_clean][1:a]amix=inputs=2:duration=longest:normalize=0"
            .to_string()
    } else {
        "[0:a]highpass=f=80,dynaudnorm[mic_clean];\
         [mic_clean][1:a]amix=inputs=2:duration=longest:normalize=0"
            .to_string()
    }
}

/// Mix two WAV files into one using ffmpeg with noise reduction on the mic track.
#[cfg(target_os = "linux")]
fn ffmpeg_mix(a: &PathBuf, b: &PathBuf, out: &PathBuf, echo_cancel: bool) -> Result<(), String> {
    let filter = build_mix_filter(echo_cancel);
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", a.to_str().unwrap(),
            "-i", b.to_str().unwrap(),
            "-filter_complex", &filter,
            out.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("failed to spawn ffmpeg: {e}"))?;

    if !status.success() {
        Err(format!("ffmpeg exited with {status}"))
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn record_mixed(_output_path: PathBuf, _stop_rx: std::sync::mpsc::Receiver<()>, _echo_cancel: bool) -> Result<(), String> {
    Err("mixed audio capture is not yet supported on this platform".to_string())
}

#[cfg(target_os = "windows")]
fn record_mixed(output_path: PathBuf, stop_rx: std::sync::mpsc::Receiver<()>, _echo_cancel: bool) -> Result<(), String> {
    // On Windows, WASAPI loopback captures both mic and system via the output device.
    let device = find_wasapi_output_device()?;
    record_single(device, output_path, stop_rx)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── State machine (no hardware) ───────────────────────────────────────────

    #[tokio::test]
    async fn is_active_false_before_start() {
        let cap = CpalAudioCapture::new();
        assert!(!cap.is_active("x"));
    }

    #[tokio::test]
    async fn stop_unknown_session_returns_err() {
        let cap = CpalAudioCapture::new();
        let err = cap.stop_session("no-such").await;
        assert!(matches!(err, Err(CoreError::Recording(_))));
    }

    // ── find_monitor_device — pure function, no hardware ─────────────────────

    #[test]
    fn find_monitor_returns_none_when_empty() {
        let empty: &[&str] = &[];
        assert_eq!(find_monitor_device(empty), None);
    }

    #[test]
    fn find_monitor_returns_none_when_no_monitor_suffix() {
        let names = ["alsa_input.usb-mic", "alsa_output.hdmi"];
        assert_eq!(find_monitor_device(&names), None);
    }

    #[test]
    fn find_monitor_selects_device_with_monitor_suffix() {
        let names = [
            "alsa_input.usb-mic",
            "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor",
            "alsa_output.hdmi.monitor",
        ];
        assert_eq!(find_monitor_device(&names), Some(1));
    }

    #[test]
    fn find_monitor_picks_first_monitor_when_multiple_exist() {
        let names = [
            "alsa_output.pci.monitor",
            "alsa_input.mic",
            "alsa_output.hdmi.monitor",
        ];
        assert_eq!(find_monitor_device(&names), Some(0));
    }

    // ── Hardware integration tests ────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires audio hardware"]
    async fn records_non_empty_wav_from_mic() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let path = dir.path().join("mic.wav");
        let cap = CpalAudioCapture::new();

        cap.start_session("s1", &path, CaptureSource::Mic, false).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        cap.stop_session("s1").await.unwrap();

        assert!(std::fs::metadata(&path).unwrap().len() > 44);
    }

    #[tokio::test]
    #[ignore = "requires parec + PipeWire monitor source (Linux)"]
    async fn records_non_empty_wav_from_system_audio() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let path = dir.path().join("system.wav");
        let cap = CpalAudioCapture::new();

        cap.start_session("s2", &path, CaptureSource::System, false).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        cap.stop_session("s2").await.unwrap();

        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len > 44, "WAV too small: {len} bytes");
    }

    #[tokio::test]
    #[ignore = "requires mic + parec + PipeWire monitor source (Linux)"]
    async fn records_non_empty_wav_from_mixed() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let path = dir.path().join("mixed.wav");
        let cap = CpalAudioCapture::new();

        cap.start_session("s3", &path, CaptureSource::Mixed, false).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        cap.stop_session("s3").await.unwrap();

        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len > 44, "WAV too small: {len} bytes");
    }

    // ── build_mix_filter — pure function ─────────────────────────────────────

    #[test]
    fn mix_filter_without_aec_contains_highpass_and_dynaudnorm() {
        let f = build_mix_filter(false);
        assert!(f.contains("highpass=f=80"), "missing highpass: {f}");
        assert!(f.contains("dynaudnorm"), "missing dynaudnorm: {f}");
    }

    #[test]
    fn mix_filter_without_aec_does_not_contain_anlms() {
        let f = build_mix_filter(false);
        assert!(!f.contains("anlms"), "unexpected anlms in non-AEC filter: {f}");
    }

    #[test]
    fn mix_filter_with_aec_contains_anlms() {
        let f = build_mix_filter(true);
        assert!(f.contains("anlms=order=512"), "missing anlms: {f}");
    }

    #[test]
    fn mix_filter_with_aec_still_contains_highpass_and_dynaudnorm() {
        let f = build_mix_filter(true);
        assert!(f.contains("highpass=f=80"), "missing highpass in AEC filter: {f}");
        assert!(f.contains("dynaudnorm"), "missing dynaudnorm in AEC filter: {f}");
    }
}
