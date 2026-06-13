use std::collections::HashMap;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use async_trait::async_trait;
// cpal drives capture on Windows/macOS; Linux goes entirely through PulseAudio
// (`parec`/`pactl`), so the cpal traits are unused there.
#[cfg(not(target_os = "linux"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(not(target_os = "linux"))]
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use meeting_core::{
    ports::{
        AudioCapture, AudioDevice, AudioDeviceEnumerator, AudioDeviceList, AudioLevel,
        AudioLevelMonitor, CaptureSource, CaptureSpec, ResolvedDevices,
    },
    CoreError,
};
use std::sync::atomic::{AtomicU32, Ordering};

struct Session {
    stop_tx: std::sync::mpsc::SyncSender<()>,
    thread: thread::JoinHandle<Result<(), String>>,
}

pub struct CpalAudioCapture {
    sessions: Mutex<HashMap<String, Session>>,
}

impl CpalAudioCapture {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
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
        spec: CaptureSpec,
    ) -> Result<ResolvedDevices, CoreError> {
        let path = output_path.to_path_buf();
        let id = session_id.to_string();
        let source = spec.source;

        // macOS: surface a denied Screen-Recording grant at *start* time
        // (clear, actionable, with a Settings deep link) instead of only when
        // the user stops the recording. System/Mixed go through
        // ScreenCaptureKit; Mic uses cpal and needs no Screen Recording right.
        #[cfg(target_os = "macos")]
        if matches!(source, CaptureSource::System | CaptureSource::Mixed) {
            super::sck_capture::preflight().map_err(CoreError::Recording)?;
        }

        // Resolve devices *now* so a missing device or pinned-but-unplugged
        // selection surfaces here (and the resolved labels go back to the UI),
        // not deep inside the capture thread.
        let resolved = resolve_devices(&spec).map_err(CoreError::Recording)?;

        let mic_name = resolved.mic_id.clone();
        let sys_name = resolved.sys_id.clone();

        let (stop_tx, stop_rx) = std::sync::mpsc::sync_channel(1);

        let thread = thread::Builder::new()
            .name(format!("cpal-recording-{id}"))
            .spawn(move || {
                record(path, stop_rx, source, spec.echo_cancel, mic_name, sys_name)
            })
            .map_err(|e| CoreError::Recording(e.to_string()))?;

        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), Session { stop_tx, thread });

        Ok(resolved.devices)
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

/// A resolved capture leg: the platform id used to open the device and the
/// human label shown to the user.
struct ResolvedLeg {
    id: String,
    label: String,
}

#[cfg(not(target_os = "linux"))]
fn default_mic(host: &cpal::Host) -> Result<cpal::Device, String> {
    host.default_input_device()
        .ok_or_else(|| "no default input device available".to_string())
}

/// Open the input device whose `Device::name()` equals `name`, falling back to
/// the OS default if `name` is `None` or no longer present. Used in the capture
/// thread; resolution (and the user-visible label) already happened at start.
#[cfg(not(target_os = "linux"))]
fn open_mic_by_name(host: &cpal::Host, name: Option<&str>) -> Result<cpal::Device, String> {
    if let Some(want) = name {
        if let Ok(devices) = host.input_devices() {
            for d in devices {
                if d.name().ok().as_deref() == Some(want) {
                    return Ok(d);
                }
            }
        }
    }
    default_mic(host)
}

// ── Device resolution (synchronous, at session start) ─────────────────────────

/// What [`resolve_devices`] produced: the labels for the UI plus the ids the
/// capture thread opens.
struct Resolved {
    devices: ResolvedDevices,
    mic_id: Option<String>,
    sys_id: Option<String>,
}

/// Resolve a [`CaptureSpec`] into the concrete devices that will be opened. A
/// pinned device that has vanished resolves back to the OS default, so the UI
/// always reflects what is truly live (ADR-1). Per-leg `None` means that leg is
/// not part of the source.
fn resolve_devices(spec: &CaptureSpec) -> Result<Resolved, String> {
    let mic = match spec.source {
        CaptureSource::Mic | CaptureSource::Mixed => {
            Some(resolve_mic_leg(spec.mic_device.as_deref())?)
        }
        CaptureSource::System => None,
    };
    let system = match spec.source {
        CaptureSource::System | CaptureSource::Mixed => {
            Some(resolve_system_leg(spec.system_device.as_deref())?)
        }
        CaptureSource::Mic => None,
    };
    Ok(Resolved {
        devices: ResolvedDevices {
            mic: mic.as_ref().map(|l| l.label.clone()),
            system: system.as_ref().map(|l| l.label.clone()),
        },
        mic_id: mic.map(|l| l.id),
        sys_id: system.map(|l| l.id),
    })
}

// ── Mic-leg resolution ────────────────────────────────────────────────────────
//
// Linux goes through PulseAudio/PipeWire (clean named sources + descriptions),
// the same backend the system leg uses. cpal's default Linux host is ALSA, which
// enumerates dozens of pseudo-PCMs (`null`, `pulse`, `hw:`, `plughw:`, `front:`,
// `dsnoop:` …) — junk no user should have to scroll through. Windows/macOS keep
// cpal, whose device names are already user-friendly.

#[cfg(target_os = "linux")]
fn resolve_mic_leg(requested: Option<&str>) -> Result<ResolvedLeg, String> {
    let sources = pulse_input_sources();
    if let Some(want) = requested {
        if let Some(s) = sources.iter().find(|s| s.name == want) {
            return Ok(ResolvedLeg {
                id: s.name.clone(),
                label: s.label(),
            });
        }
    }
    if let Some(def) = default_pulse_source() {
        let label = sources
            .iter()
            .find(|s| s.name == def)
            .map(|s| s.label())
            .unwrap_or_else(|| def.clone());
        return Ok(ResolvedLeg { id: def, label });
    }
    sources
        .into_iter()
        .next()
        .map(|s| ResolvedLeg {
            label: s.label(),
            id: s.name,
        })
        .ok_or_else(|| "no input source available".to_string())
}

#[cfg(not(target_os = "linux"))]
fn resolve_mic_leg(requested: Option<&str>) -> Result<ResolvedLeg, String> {
    let host = cpal::default_host();
    if let Some(want) = requested {
        if let Ok(devices) = host.input_devices() {
            for d in devices {
                if d.name().ok().as_deref() == Some(want) {
                    return Ok(ResolvedLeg {
                        id: want.to_string(),
                        label: want.to_string(),
                    });
                }
            }
        }
    }
    let name = default_mic(&host)?.name().map_err(|e| e.to_string())?;
    Ok(ResolvedLeg {
        id: name.clone(),
        label: name,
    })
}

// ── System-leg resolution ─────────────────────────────────────────────────────

/// macOS has no per-output handle (ScreenCaptureKit captures the aggregate mix).
#[cfg(target_os = "macos")]
fn resolve_system_leg(_requested: Option<&str>) -> Result<ResolvedLeg, String> {
    Ok(ResolvedLeg {
        id: String::new(),
        label: "System audio".to_string(),
    })
}

#[cfg(target_os = "linux")]
fn resolve_system_leg(requested: Option<&str>) -> Result<ResolvedLeg, String> {
    let monitors = pulse_monitor_sources();
    if let Some(want) = requested {
        if let Some(s) = monitors.iter().find(|s| s.name == want) {
            return Ok(ResolvedLeg {
                id: s.name.clone(),
                label: s.label(),
            });
        }
    }
    let def = find_pulseaudio_monitor()?;
    let label = monitors
        .iter()
        .find(|s| s.name == def)
        .map(|s| s.label())
        .unwrap_or_else(|| def.clone());
    Ok(ResolvedLeg { id: def, label })
}

#[cfg(target_os = "windows")]
fn resolve_system_leg(requested: Option<&str>) -> Result<ResolvedLeg, String> {
    if let Some(want) = requested {
        let wasapi = cpal::host_from_id(cpal::HostId::Wasapi)
            .map_err(|e| format!("WASAPI host unavailable: {e}"))?;
        if let Ok(devices) = wasapi.output_devices() {
            for d in devices {
                if d.name().ok().as_deref() == Some(want) {
                    return Ok(ResolvedLeg {
                        id: want.to_string(),
                        label: want.to_string(),
                    });
                }
            }
        }
    }
    let name = find_wasapi_output_device()?
        .name()
        .map_err(|e| e.to_string())?;
    Ok(ResolvedLeg {
        id: name.clone(),
        label: name,
    })
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn resolve_system_leg(_requested: Option<&str>) -> Result<ResolvedLeg, String> {
    Err("system audio capture is not yet supported on this platform".to_string())
}

// ── PulseAudio/PipeWire source enumeration (Linux) ────────────────────────────

/// One PulseAudio/PipeWire source parsed from `pactl list sources`.
#[cfg(target_os = "linux")]
struct PulseSource {
    name: String,
    description: Option<String>,
    monitor: bool,
}

#[cfg(target_os = "linux")]
impl PulseSource {
    /// Human label: the `Description` if present, else the raw node name.
    fn label(&self) -> String {
        self.description.clone().unwrap_or_else(|| self.name.clone())
    }
}

/// Parse `pactl list sources` (verbose) into sources with their descriptions and
/// monitor flag. Pure — unit-tested without audio hardware.
#[cfg(target_os = "linux")]
fn parse_pulse_sources(text: &str) -> Vec<PulseSource> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut monitor = false;

    let mut flush = |name: &mut Option<String>,
                     description: &mut Option<String>,
                     monitor: &mut bool| {
        if let Some(n) = name.take() {
            let is_monitor = *monitor || n.contains(".monitor");
            out.push(PulseSource {
                name: n,
                description: description.take(),
                monitor: is_monitor,
            });
        }
        *monitor = false;
    };

    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("Source #") {
            flush(&mut name, &mut description, &mut monitor);
        } else if let Some(rest) = t.strip_prefix("Name: ") {
            name = Some(rest.trim().to_string());
        } else if let Some(rest) = t.strip_prefix("Description: ") {
            description = Some(rest.trim().to_string());
        } else if let Some(rest) = t.strip_prefix("Monitor of Sink: ") {
            if rest.trim() != "n/a" {
                monitor = true;
            }
        }
    }
    flush(&mut name, &mut description, &mut monitor);
    out
}

#[cfg(target_os = "linux")]
fn pulse_sources() -> Vec<PulseSource> {
    // Force the C locale: `pactl list sources` translates its field keys
    // (`Description:` → `Описание:` …) under a non-English locale, which would
    // make the parser match nothing. Descriptions stay human-readable.
    match std::process::Command::new("pactl")
        .env("LC_ALL", "C")
        .args(["list", "sources"])
        .output()
    {
        Ok(o) => parse_pulse_sources(&String::from_utf8_lossy(&o.stdout)),
        Err(_) => Vec::new(),
    }
}

/// Real microphone inputs (everything that is not a monitor/loopback source).
#[cfg(target_os = "linux")]
fn pulse_input_sources() -> Vec<PulseSource> {
    pulse_sources().into_iter().filter(|s| !s.monitor).collect()
}

/// System-audio loopback sources (`.monitor` of each sink).
#[cfg(target_os = "linux")]
fn pulse_monitor_sources() -> Vec<PulseSource> {
    pulse_sources().into_iter().filter(|s| s.monitor).collect()
}

/// The current default PulseAudio/PipeWire input source name, if any.
#[cfg(target_os = "linux")]
fn default_pulse_source() -> Option<String> {
    std::process::Command::new("pactl")
        .args(["get-default-source"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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
    let idx = find_monitor_device(&names).ok_or_else(|| {
        format!(
            "no monitor source found via pactl (sources: {})",
            names.join(", ")
        )
    })?;

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

/// Open the WASAPI output (loopback) device by name, falling back to default.
#[cfg(target_os = "windows")]
fn open_output_by_name(name: Option<&str>) -> Result<cpal::Device, String> {
    if let Some(want) = name {
        let wasapi = cpal::host_from_id(cpal::HostId::Wasapi)
            .map_err(|e| format!("WASAPI host unavailable: {e}"))?;
        if let Ok(devices) = wasapi.output_devices() {
            for d in devices {
                if d.name().ok().as_deref() == Some(want) {
                    return Ok(d);
                }
            }
        }
    }
    find_wasapi_output_device()
}

// ── Recording threads ─────────────────────────────────────────────────────────

fn record(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    source: CaptureSource,
    echo_cancel: bool,
    mic_name: Option<String>,
    sys_name: Option<String>,
) -> Result<(), String> {
    match source {
        CaptureSource::Mic => record_mic(output_path, stop_rx, mic_name),
        CaptureSource::System => record_system(output_path, stop_rx, sys_name),
        CaptureSource::Mixed => record_mixed(output_path, stop_rx, echo_cancel, mic_name, sys_name),
    }
}

/// Capture a single microphone. Linux uses PulseAudio/PipeWire (`parec`) for the
/// same clean device set the picker shows; other platforms use cpal.
#[cfg(target_os = "linux")]
fn record_mic(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    mic_name: Option<String>,
) -> Result<(), String> {
    let source = match mic_name {
        Some(name) => name,
        None => default_pulse_source().ok_or_else(|| "no default input source".to_string())?,
    };
    record_parec(&source, output_path, stop_rx)
}

#[cfg(not(target_os = "linux"))]
fn record_mic(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    mic_name: Option<String>,
) -> Result<(), String> {
    let host = cpal::default_host();
    record_single(open_mic_by_name(&host, mic_name.as_deref())?, output_path, stop_rx)
}

// ── cpal sample-format handling (Windows/macOS) ───────────────────────────────
//
// A cpal input device's native format is not always f32 — WASAPI microphones
// commonly capture as i16. `build_input_stream::<f32>` on such a device fails
// with StreamConfigNotSupported, so capture/metering must open the stream with
// the device's real format and convert each sample to f32.

/// Convert one sample of any cpal format to `f32` (the float WAV sample type).
#[cfg(not(target_os = "linux"))]
fn to_f32<T>(s: T) -> f32
where
    f32: FromSample<T>,
{
    f32::from_sample(s)
}

/// A shared float-WAV writer handle the cpal data callback appends to.
#[cfg(not(target_os = "linux"))]
type WavWriterHandle = Arc<Mutex<hound::WavWriter<BufWriter<std::fs::File>>>>;

/// Build a WAV-writing input stream for samples of type `T`, converting each to
/// f32. Generic over the device's native format so non-f32 devices work too.
#[cfg(not(target_os = "linux"))]
fn wav_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    writer: WavWriterHandle,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let mut w = writer.lock().unwrap();
            for &s in data {
                let _ = w.write_sample(to_f32(s));
            }
        },
        |err| tracing::error!("cpal input stream error: {err}"),
        None,
    )
}

/// Open a WAV-writing input stream, dispatching on the device's runtime format.
#[cfg(not(target_os = "linux"))]
fn build_wav_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    writer: WavWriterHandle,
) -> Result<cpal::Stream, String> {
    match format {
        SampleFormat::I8 => wav_input_stream::<i8>(device, config, writer),
        SampleFormat::I16 => wav_input_stream::<i16>(device, config, writer),
        SampleFormat::I32 => wav_input_stream::<i32>(device, config, writer),
        SampleFormat::I64 => wav_input_stream::<i64>(device, config, writer),
        SampleFormat::U8 => wav_input_stream::<u8>(device, config, writer),
        SampleFormat::U16 => wav_input_stream::<u16>(device, config, writer),
        SampleFormat::U32 => wav_input_stream::<u32>(device, config, writer),
        SampleFormat::U64 => wav_input_stream::<u64>(device, config, writer),
        SampleFormat::F32 => wav_input_stream::<f32>(device, config, writer),
        SampleFormat::F64 => wav_input_stream::<f64>(device, config, writer),
        other => return Err(format!("unsupported input sample format: {other:?}")),
    }
    .map_err(|e| e.to_string())
}

/// Build a level-metering input stream for samples of type `T` (no file).
#[cfg(not(target_os = "linux"))]
fn meter_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let peak = data
                .iter()
                .fold(0.0_f32, |m, &s| m.max(to_f32(s).abs()))
                .min(1.0);
            level.store(peak.to_bits(), Ordering::Relaxed);
        },
        |err| tracing::error!("cpal monitor stream error: {err}"),
        None,
    )
}

/// Open a level-metering input stream, dispatching on the device's format.
#[cfg(not(target_os = "linux"))]
fn build_meter_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream, String> {
    match format {
        SampleFormat::I8 => meter_input_stream::<i8>(device, config, level),
        SampleFormat::I16 => meter_input_stream::<i16>(device, config, level),
        SampleFormat::I32 => meter_input_stream::<i32>(device, config, level),
        SampleFormat::I64 => meter_input_stream::<i64>(device, config, level),
        SampleFormat::U8 => meter_input_stream::<u8>(device, config, level),
        SampleFormat::U16 => meter_input_stream::<u16>(device, config, level),
        SampleFormat::U32 => meter_input_stream::<u32>(device, config, level),
        SampleFormat::U64 => meter_input_stream::<u64>(device, config, level),
        SampleFormat::F32 => meter_input_stream::<f32>(device, config, level),
        SampleFormat::F64 => meter_input_stream::<f64>(device, config, level),
        other => return Err(format!("unsupported input sample format: {other:?}")),
    }
    .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
fn record_single(
    device: cpal::Device,
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // A microphone is opened as an input device, but the Windows System/Mixed
    // legs open an *output* device for WASAPI loopback — which has no input
    // config (`default_input_config` then returns StreamTypeNotSupported, i.e.
    // "stream type is not supported by the device"). Fall back to the output
    // (loopback) format so system-audio capture works.
    let config = device
        .default_input_config()
        .or_else(|_| device.default_output_config())
        .map_err(|e| e.to_string())?;
    let sample_format = config.sample_format();

    let spec = hound::WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let stream_config: cpal::StreamConfig = config.into();

    let file = std::fs::File::create(&output_path).map_err(|e| e.to_string())?;
    let writer = Arc::new(Mutex::new(
        hound::WavWriter::new(BufWriter::new(file), spec).map_err(|e| e.to_string())?,
    ));

    // The device's native capture format is not always f32 (WASAPI mics are
    // commonly i16). Building an f32 stream on a non-f32 device fails with
    // StreamConfigNotSupported, so dispatch on the real format and convert each
    // sample to f32 for the float WAV.
    let writer_cb = Arc::clone(&writer);
    let stream = build_wav_input_stream(&device, &stream_config, sample_format, writer_cb)?;

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
fn record_system(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    sys_name: Option<String>,
) -> Result<(), String> {
    let monitor = match sys_name {
        Some(name) => name,
        None => find_pulseaudio_monitor()?,
    };
    record_parec(&monitor, output_path, stop_rx)
}

#[cfg(target_os = "windows")]
fn record_system(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    sys_name: Option<String>,
) -> Result<(), String> {
    let device = open_output_by_name(sys_name.as_deref())?;
    record_single(device, output_path, stop_rx)
}

/// macOS system audio via ScreenCaptureKit (audio-only). See `sck_capture`.
/// `sys_name` is ignored — SCK captures the aggregate mix, not a chosen output.
#[cfg(target_os = "macos")]
fn record_system(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    _sys_name: Option<String>,
) -> Result<(), String> {
    super::sck_capture::record_system(output_path, stop_rx)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn record_system(
    _output_path: PathBuf,
    _stop_rx: std::sync::mpsc::Receiver<()>,
    _sys_name: Option<String>,
) -> Result<(), String> {
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
            "--device",
            source_name,
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
fn record_mixed(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    echo_cancel: bool,
    mic_name: Option<String>,
    sys_name: Option<String>,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let monitor = match sys_name {
        Some(name) => name,
        None => find_pulseaudio_monitor()?,
    };
    let mic_source = match mic_name {
        Some(name) => name,
        None => default_pulse_source().ok_or_else(|| "no default input source".to_string())?,
    };

    let mic_tmp = output_path.with_extension("mic_tmp.wav");
    let sys_tmp = output_path.with_extension("sys_tmp.wav");

    let (mic_stop_tx, mic_stop_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let (sys_stop_tx, sys_stop_rx) = std::sync::mpsc::sync_channel::<()>(1);

    let mic_path = mic_tmp.clone();
    let sys_path = sys_tmp.clone();
    let monitor_name = monitor.clone();

    // Both legs now go through parec (same 48 kHz PulseAudio clock domain), so
    // the separate temp files exist only to keep two writers off one lock.
    let mic_thread = thread::spawn(move || record_parec(&mic_source, mic_path, mic_stop_rx));

    let sys_thread = thread::spawn(move || record_parec(&monitor_name, sys_path, sys_stop_rx));

    let _ = stop_rx.recv();
    let _ = mic_stop_tx.send(());
    let _ = sys_stop_tx.send(());

    mic_thread
        .join()
        .map_err(|_| "mic thread panicked".to_string())??;
    sys_thread
        .join()
        .map_err(|_| "sys thread panicked".to_string())??;

    ffmpeg_mix(
        &mic_tmp,
        &sys_tmp,
        &output_path,
        &build_mix_filter(echo_cancel),
    )?;

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

/// mic+system mix filter for backends whose two legs run on **independent audio
/// clocks**: macOS (ScreenCaptureKit + cpal) and Windows (WASAPI loopback + cpal
/// mic). Unlike the Linux path (both legs via `parec`, one PulseAudio clock), a
/// plain `amix` would accumulate skew, and the two legs can also differ in
/// sample rate and channel count (e.g. a mono mic + stereo loopback). So each
/// input is passed through `aresample=async=1` — which continuously
/// stretches/squeezes the stream to its presentation timestamps
/// (`min_hard_comp=0.100` bounds a single correction so speech is not
/// pitch-warped) — then `aformat` to a common rate/layout/format so `amix` and
/// `anlms` receive matching inputs.
///
/// In the AEC branch the system track is consumed twice (echo reference +
/// final mix), so it is `asplit`; a filter-graph pad cannot be reused
/// (unlike a raw input specifier such as `[1:a]`).
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) fn build_mix_filter_resync(echo_cancel: bool) -> String {
    let resync = "aresample=async=1:min_hard_comp=0.100:first_pts=0,\
                  aformat=sample_rates=48000:channel_layouts=stereo:sample_fmts=fltp";
    if echo_cancel {
        format!(
            "[0:a]{resync}[mic_r];\
             [1:a]{resync},asplit=2[sys_a][sys_b];\
             [mic_r][sys_a]anlms=order=512:mu=0.05:eps=1[mic_aec];\
             [mic_aec]highpass=f=80,dynaudnorm[mic_clean];\
             [mic_clean][sys_b]amix=inputs=2:duration=longest:normalize=0"
        )
    } else {
        format!(
            "[0:a]{resync}[mic_r];\
             [1:a]{resync}[sys_r];\
             [mic_r]highpass=f=80,dynaudnorm[mic_clean];\
             [mic_clean][sys_r]amix=inputs=2:duration=longest:normalize=0"
        )
    }
}

/// Resolve which `ffmpeg` to run, given a directory to look in first.
///
/// Packaged apps ship an `ffmpeg` next to the `meeting-server` binary so mixed
/// recording is self-contained; a dev build (or a system install) has none
/// there and falls back to `ffmpeg` on PATH. Pure (takes the dir) so it is
/// testable without a real install layout.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn resolve_ffmpeg_in(dir: &Path) -> std::ffi::OsString {
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let bundled = dir.join(name);
    if bundled.is_file() {
        bundled.into_os_string()
    } else {
        std::ffi::OsString::from("ffmpeg")
    }
}

/// The `ffmpeg` program to spawn: bundled-next-to-the-sidecar if present, else
/// PATH. See [`resolve_ffmpeg_in`].
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn ffmpeg_program() -> std::ffi::OsString {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(resolve_ffmpeg_in))
        .unwrap_or_else(|| std::ffi::OsString::from("ffmpeg"))
}

/// Mix two WAV files into one using ffmpeg with the given `-filter_complex`.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn ffmpeg_mix(a: &PathBuf, b: &PathBuf, out: &PathBuf, filter: &str) -> Result<(), String> {
    let status = std::process::Command::new(ffmpeg_program())
        .args([
            "-y",
            "-i",
            a.to_str().unwrap(),
            "-i",
            b.to_str().unwrap(),
            "-filter_complex",
            filter,
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

/// macOS mixed capture: mic via cpal + system via ScreenCaptureKit, each to a
/// separate temp WAV, then mixed by ffmpeg with clock-drift compensation
/// (`build_mix_filter_resync`). The two backends run on independent audio
/// clocks; the resample-to-PTS in the mix filter aligns them.
#[cfg(target_os = "macos")]
fn record_mixed(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    echo_cancel: bool,
    mic_name: Option<String>,
    _sys_name: Option<String>,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mic_tmp = output_path.with_extension("mic_tmp.wav");
    let sys_tmp = output_path.with_extension("sys_tmp.wav");

    let (mic_stop_tx, mic_stop_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let (sys_stop_tx, sys_stop_rx) = std::sync::mpsc::sync_channel::<()>(1);

    let mic_path = mic_tmp.clone();
    let sys_path = sys_tmp.clone();

    let mic_thread = thread::spawn(move || {
        let host = cpal::default_host();
        let mic = open_mic_by_name(&host, mic_name.as_deref())?;
        record_single(mic, mic_path, mic_stop_rx)
    });

    let sys_thread =
        thread::spawn(move || super::sck_capture::record_system(sys_path, sys_stop_rx));

    let _ = stop_rx.recv();
    let _ = mic_stop_tx.send(());
    let _ = sys_stop_tx.send(());

    mic_thread
        .join()
        .map_err(|_| "mic thread panicked".to_string())??;
    sys_thread
        .join()
        .map_err(|_| "sys thread panicked".to_string())??;

    ffmpeg_mix(
        &mic_tmp,
        &sys_tmp,
        &output_path,
        &build_mix_filter_resync(echo_cancel),
    )?;

    let _ = std::fs::remove_file(&mic_tmp);
    let _ = std::fs::remove_file(&sys_tmp);

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn record_mixed(
    _output_path: PathBuf,
    _stop_rx: std::sync::mpsc::Receiver<()>,
    _echo_cancel: bool,
    _mic_name: Option<String>,
    _sys_name: Option<String>,
) -> Result<(), String> {
    Err("mixed audio capture is not yet supported on this platform".to_string())
}

/// Windows mixed capture: mic via cpal (input device) + system via WASAPI
/// loopback (output device), each to a separate temp WAV, then mixed by ffmpeg.
///
/// WASAPI loopback records only what is *rendered* to the output endpoint — it
/// does NOT include the microphone — so a single loopback stream cannot be the
/// mix (the earlier one-device approach silently dropped the mic). The two legs
/// run on independent device clocks and can differ in rate/channels, so they go
/// through [`build_mix_filter_resync`] (resample-to-PTS + rate/layout
/// normalisation) like the macOS path. Requires the `ffmpeg` CLI on PATH.
#[cfg(target_os = "windows")]
fn record_mixed(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
    echo_cancel: bool,
    mic_name: Option<String>,
    sys_name: Option<String>,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mic_tmp = output_path.with_extension("mic_tmp.wav");
    let sys_tmp = output_path.with_extension("sys_tmp.wav");

    let (mic_stop_tx, mic_stop_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let (sys_stop_tx, sys_stop_rx) = std::sync::mpsc::sync_channel::<()>(1);

    let mic_path = mic_tmp.clone();
    let sys_path = sys_tmp.clone();

    let mic_thread = thread::spawn(move || {
        let host = cpal::default_host();
        let mic = open_mic_by_name(&host, mic_name.as_deref())?;
        record_single(mic, mic_path, mic_stop_rx)
    });
    let sys_thread = thread::spawn(move || {
        let dev = open_output_by_name(sys_name.as_deref())?;
        record_single(dev, sys_path, sys_stop_rx)
    });

    let _ = stop_rx.recv();
    let _ = mic_stop_tx.send(());
    let _ = sys_stop_tx.send(());

    let mic_res = mic_thread
        .join()
        .map_err(|_| "mic thread panicked".to_string())?;
    let sys_res = sys_thread
        .join()
        .map_err(|_| "sys thread panicked".to_string())?;

    // Clean up temp files even if a leg failed or the mix can't run.
    let cleanup = || {
        let _ = std::fs::remove_file(&mic_tmp);
        let _ = std::fs::remove_file(&sys_tmp);
    };

    if let Err(e) = mic_res {
        cleanup();
        return Err(e);
    }
    if let Err(e) = sys_res {
        cleanup();
        return Err(e);
    }

    let mix = ffmpeg_mix(
        &mic_tmp,
        &sys_tmp,
        &output_path,
        &build_mix_filter_resync(echo_cancel),
    );
    cleanup();
    mix
}

// ── Device enumeration ────────────────────────────────────────────────────────

/// Lists capture devices via cpal (mics) and the platform loopback backend
/// (system outputs). Stateless; the same name strings it returns are what
/// [`CpalAudioCapture`] resolves against at session start.
#[derive(Default)]
pub struct CpalAudioDevices;

impl CpalAudioDevices {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AudioDeviceEnumerator for CpalAudioDevices {
    async fn list_devices(&self) -> Result<AudioDeviceList, CoreError> {
        // pactl/cpal enumeration is blocking (a subprocess on Linux); keep it
        // off the async runtime's worker threads.
        tokio::task::spawn_blocking(|| {
            Ok(AudioDeviceList {
                input: enumerate_inputs(),
                output: enumerate_outputs(),
                system_selectable: SYSTEM_SELECTABLE,
            })
        })
        .await
        .map_err(|e| CoreError::Recording(format!("device enumeration failed: {e}")))?
    }
}

/// Microphone inputs. Linux uses PulseAudio/PipeWire sources (clean named
/// devices with descriptions) instead of cpal's noisy ALSA PCM list.
#[cfg(target_os = "linux")]
fn enumerate_inputs() -> Vec<AudioDevice> {
    let default = default_pulse_source();
    pulse_input_sources()
        .into_iter()
        .map(|s| AudioDevice {
            is_default: default.as_deref() == Some(s.name.as_str()),
            label: s.label(),
            id: s.name,
        })
        .collect()
}

#[cfg(not(target_os = "linux"))]
fn enumerate_inputs() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                if seen.insert(name.clone()) {
                    out.push(AudioDevice {
                        is_default: default.as_deref() == Some(name.as_str()),
                        label: name.clone(),
                        id: name,
                    });
                }
            }
        }
    }
    out
}

#[cfg(target_os = "linux")]
const SYSTEM_SELECTABLE: bool = true;
#[cfg(target_os = "windows")]
const SYSTEM_SELECTABLE: bool = true;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
const SYSTEM_SELECTABLE: bool = false;

#[cfg(target_os = "linux")]
fn enumerate_outputs() -> Vec<AudioDevice> {
    let default = find_pulseaudio_monitor().ok();
    pulse_monitor_sources()
        .into_iter()
        .map(|s| AudioDevice {
            is_default: default.as_deref() == Some(s.name.as_str()),
            label: s.label(),
            id: s.name,
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn enumerate_outputs() -> Vec<AudioDevice> {
    let wasapi = match cpal::host_from_id(cpal::HostId::Wasapi) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let default = wasapi.default_output_device().and_then(|d| d.name().ok());
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    if let Ok(devices) = wasapi.output_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                if seen.insert(name.clone()) {
                    out.push(AudioDevice {
                        is_default: default.as_deref() == Some(name.as_str()),
                        label: name.clone(),
                        id: name,
                    });
                }
            }
        }
    }
    out
}

/// macOS (ScreenCaptureKit) has no per-output handle; no selectable list.
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn enumerate_outputs() -> Vec<AudioDevice> {
    Vec::new()
}

// ── Live level monitor (device test) ──────────────────────────────────────────

struct MonitorSession {
    stop_tx: std::sync::mpsc::SyncSender<()>,
    thread: thread::JoinHandle<Result<(), String>>,
    /// Latest linear peak (0.0–1.0) as `f32` bits, updated by the capture thread.
    level: Arc<AtomicU32>,
}

/// Captures one device without writing a file and publishes a live input level
/// for the settings device-test meter. Reuses the same device resolution and
/// capture backends as recording (PulseAudio on Linux, cpal elsewhere).
#[derive(Default)]
pub struct CpalLevelMonitor {
    sessions: Mutex<HashMap<String, MonitorSession>>,
}

impl CpalLevelMonitor {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl AudioLevelMonitor for CpalLevelMonitor {
    async fn start(&self, id: &str, spec: CaptureSpec) -> Result<ResolvedDevices, CoreError> {
        if matches!(spec.source, CaptureSource::Mixed) {
            return Err(CoreError::Recording(
                "level test captures one source at a time".to_string(),
            ));
        }

        #[cfg(target_os = "macos")]
        if matches!(spec.source, CaptureSource::System) {
            return Err(CoreError::Recording(
                "system-audio test is not available on macOS".to_string(),
            ));
        }

        let resolved = resolve_devices(&spec).map_err(CoreError::Recording)?;
        let source = spec.source;
        let mic_id = resolved.mic_id.clone();
        let sys_id = resolved.sys_id.clone();

        let level = Arc::new(AtomicU32::new(0));
        let level_thread = Arc::clone(&level);
        let (stop_tx, stop_rx) = std::sync::mpsc::sync_channel(1);

        let thread = thread::Builder::new()
            .name(format!("audio-monitor-{id}"))
            .spawn(move || match source {
                CaptureSource::Mic => run_mic_monitor(mic_id, level_thread, stop_rx),
                CaptureSource::System => run_system_monitor(sys_id, level_thread, stop_rx),
                CaptureSource::Mixed => unreachable!("guarded above"),
            })
            .map_err(|e| CoreError::Recording(e.to_string()))?;

        // Replace any previous session under the same id (e.g. switching legs).
        if let Some(prev) = self.sessions.lock().unwrap().insert(
            id.to_string(),
            MonitorSession {
                stop_tx,
                thread,
                level,
            },
        ) {
            let _ = prev.stop_tx.send(());
            let _ = prev.thread.join();
        }

        Ok(resolved.devices)
    }

    fn level(&self, id: &str) -> Option<AudioLevel> {
        let sessions = self.sessions.lock().unwrap();
        let s = sessions.get(id)?;
        let peak = f32::from_bits(s.level.load(Ordering::Relaxed)).clamp(0.0, 1.0);
        let peak_db = if peak > 0.0 {
            (20.0 * peak.log10()).max(-60.0)
        } else {
            -60.0
        };
        Some(AudioLevel {
            level: peak,
            peak_db,
        })
    }

    async fn stop(&self, id: &str) -> Result<(), CoreError> {
        let session = self
            .sessions
            .lock()
            .unwrap()
            .remove(id)
            .ok_or_else(|| CoreError::Recording(format!("monitor not found: {id}")))?;
        let _ = session.stop_tx.send(());
        session
            .thread
            .join()
            .map_err(|_| CoreError::Recording("monitor thread panicked".into()))?
            .map_err(CoreError::Recording)
    }
}

/// Update `level` with the peak amplitude of `samples` (max |s|, clamped 0–1).
/// Linux meters parec's f32 byte stream; cpal (Windows/macOS) computes the peak
/// inline in [`meter_input_stream`] after converting the device's native format.
#[cfg(target_os = "linux")]
fn publish_peak(level: &AtomicU32, samples: &[f32]) {
    let peak = samples
        .iter()
        .fold(0.0_f32, |m, &s| m.max(s.abs()))
        .min(1.0);
    level.store(peak.to_bits(), Ordering::Relaxed);
}

// ── Mic monitor ───────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn run_mic_monitor(
    mic_name: Option<String>,
    level: Arc<AtomicU32>,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    let source = match mic_name {
        Some(name) => name,
        None => default_pulse_source().ok_or_else(|| "no default input source".to_string())?,
    };
    monitor_parec(&source, level, stop_rx)
}

#[cfg(not(target_os = "linux"))]
fn run_mic_monitor(
    mic_name: Option<String>,
    level: Arc<AtomicU32>,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    let host = cpal::default_host();
    monitor_cpal(open_mic_by_name(&host, mic_name.as_deref())?, level, stop_rx)
}

// ── System monitor ────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn run_system_monitor(
    sys_name: Option<String>,
    level: Arc<AtomicU32>,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    let source = match sys_name {
        Some(name) => name,
        None => find_pulseaudio_monitor()?,
    };
    monitor_parec(&source, level, stop_rx)
}

#[cfg(target_os = "windows")]
fn run_system_monitor(
    sys_name: Option<String>,
    level: Arc<AtomicU32>,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    monitor_cpal(open_output_by_name(sys_name.as_deref())?, level, stop_rx)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn run_system_monitor(
    _sys_name: Option<String>,
    _level: Arc<AtomicU32>,
    _stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    Err("system-audio test is not available on this platform".to_string())
}

// ── Capture backends (level only, no file) ────────────────────────────────────

/// Meter a PulseAudio/PipeWire source via `parec`: mono 16 kHz is plenty for a
/// level readout and cheap on CPU.
#[cfg(target_os = "linux")]
fn monitor_parec(
    source_name: &str,
    level: Arc<AtomicU32>,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    use std::io::Read;

    let mut child = std::process::Command::new("parec")
        .args([
            "--device",
            source_name,
            "--format=float32le",
            "--channels=1",
            "--rate=16000",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn parec: {e}"))?;

    let mut out = child.stdout.take().unwrap();
    let level_reader = Arc::clone(&level);

    let read_thread = thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let samples: Vec<f32> = buf[..n]
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    publish_peak(&level_reader, &samples);
                }
            }
        }
    });

    let _ = stop_rx.recv();
    child.kill().ok();
    child.wait().ok();
    read_thread.join().ok();
    level.store(0.0_f32.to_bits(), Ordering::Relaxed);
    Ok(())
}

/// Meter a cpal device (mic input, or a Windows output endpoint in loopback).
#[cfg(not(target_os = "linux"))]
fn monitor_cpal(
    device: cpal::Device,
    level: Arc<AtomicU32>,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    // Output devices (Windows system-audio loopback) have no input config; fall
    // back to their output format, same as the recording path.
    let config = device
        .default_input_config()
        .or_else(|_| device.default_output_config())
        .map_err(|e| e.to_string())?;
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();
    let level_cb = Arc::clone(&level);
    // Match the device's real format (often i16 on WASAPI), converting to f32.
    let stream = build_meter_input_stream(&device, &stream_config, sample_format, level_cb)?;
    stream.play().map_err(|e| e.to_string())?;
    let _ = stop_rx.recv();
    drop(stream);
    level.store(0.0_f32.to_bits(), Ordering::Relaxed);
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mic_spec() -> CaptureSpec {
        CaptureSpec {
            source: CaptureSource::Mic,
            ..Default::default()
        }
    }
    fn system_spec() -> CaptureSpec {
        CaptureSpec {
            source: CaptureSource::System,
            ..Default::default()
        }
    }
    fn mixed_spec() -> CaptureSpec {
        CaptureSpec {
            source: CaptureSource::Mixed,
            ..Default::default()
        }
    }

    // ── resolve_devices — leg presence per source (no hardware on resolution
    //    shape; mic resolution needs a device so we only assert leg presence) ──

    #[test]
    fn resolve_devices_mic_only_has_no_system_leg() {
        // Mic resolution can fail without hardware; only assert the system leg
        // is absent for a mic-only spec (a pure structural property).
        if let Ok(r) = resolve_devices(&mic_spec()) {
            assert!(
                r.devices.system.is_none() && r.sys_id.is_none(),
                "mic-only must not resolve a system leg"
            );
        }
    }

    #[test]
    fn system_selectable_const_matches_platform() {
        let expected = cfg!(any(target_os = "linux", target_os = "windows"));
        assert_eq!(SYSTEM_SELECTABLE, expected);
    }

    // ── pactl source parsing — keeps ALSA junk out of the mic list ────────────

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_pulse_sources_splits_inputs_from_monitors_with_labels() {
        // Trimmed `pactl list sources` shape: one real mic, one monitor.
        let text = "\
Source #1
\tName: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
\tDescription: Monitor of Built-in Audio Analog Stereo
\tMonitor of Sink: alsa_output.pci-0000_00_1f.3.analog-stereo
Source #2
\tName: alsa_input.pci-0000_00_1f.3.analog-stereo
\tDescription: Built-in Audio Analog Stereo
\tMonitor of Sink: n/a
";
        let sources = parse_pulse_sources(text);
        assert_eq!(sources.len(), 2);

        let inputs: Vec<_> = sources.iter().filter(|s| !s.monitor).collect();
        let monitors: Vec<_> = sources.iter().filter(|s| s.monitor).collect();
        assert_eq!(inputs.len(), 1, "exactly one real mic");
        assert_eq!(inputs[0].name, "alsa_input.pci-0000_00_1f.3.analog-stereo");
        assert_eq!(inputs[0].label(), "Built-in Audio Analog Stereo");
        assert_eq!(monitors.len(), 1, "exactly one monitor");
        assert!(monitors[0].monitor);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_pulse_sources_falls_back_to_name_without_description() {
        let text = "Source #0\n\tName: some.weird.source\n\tMonitor of Sink: n/a\n";
        let sources = parse_pulse_sources(text);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].label(), "some.weird.source");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_pulse_sources_empty_is_empty() {
        assert!(parse_pulse_sources("").is_empty());
    }

    // Manual diagnostic: meter the default mic for ~1.5 s and print the level.
    //   cargo test -p meeting-adapters monitor_mic_level -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "captures from the mic; run manually"]
    async fn monitor_mic_level() {
        let mon = CpalLevelMonitor::new();
        mon.start("probe", mic_spec()).await.unwrap();
        for _ in 0..6 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let l = mon.level("probe").unwrap();
            println!("level = {:.3}  ({:.1} dB)", l.level, l.peak_db);
        }
        mon.stop("probe").await.unwrap();
        assert!(mon.level("probe").is_none(), "session must clear on stop");
    }

    // Manual diagnostic: print the device lists this machine actually exposes.
    //   cargo test -p meeting-adapters print_enumerated_devices -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "prints local audio devices; run manually"]
    async fn print_enumerated_devices() {
        let list = CpalAudioDevices::new().list_devices().await.unwrap();
        println!("system_selectable = {}", list.system_selectable);
        println!("── inputs ({}) ──", list.input.len());
        for d in &list.input {
            println!("  {}{}", d.label, if d.is_default { "  [default]" } else { "" });
        }
        println!("── outputs ({}) ──", list.output.len());
        for d in &list.output {
            println!("  {}{}", d.label, if d.is_default { "  [default]" } else { "" });
        }
    }

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

    // ── cpal sample-format conversion (Windows/macOS capture) ─────────────────
    //
    // Regression for "The requested stream type is not supported by the device":
    // WASAPI mics commonly capture as i16, but capture built an f32 stream
    // unconditionally (StreamConfigNotSupported). The fix opens the stream in the
    // device's real format and converts each sample to f32 — verify that
    // conversion maps integer full-scale to ~±1.0 so the recorded WAV is sane.
    // (The end-to-end "i16 device records audio" contract is hardware-bound and
    // covered by the per-OS smoke checklist.)
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn to_f32_maps_sample_formats_to_unit_range() {
        assert!((to_f32(i16::MAX) - 1.0).abs() < 1e-3, "i16::MAX -> ~1.0");
        assert!((to_f32(i16::MIN) + 1.0).abs() < 1e-3, "i16::MIN -> ~-1.0");
        assert!(to_f32(0_i16).abs() < 1e-6, "0i16 -> 0.0");
        assert!((to_f32(i32::MAX) - 1.0).abs() < 1e-3, "i32::MAX -> ~1.0");
        assert!((to_f32(1.0_f32) - 1.0).abs() < 1e-9, "f32 is identity");
    }

    // ── Hardware integration tests ────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires audio hardware"]
    async fn records_non_empty_wav_from_mic() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let path = dir.path().join("mic.wav");
        let cap = CpalAudioCapture::new();

        cap.start_session("s1", &path, mic_spec())
            .await
            .unwrap();
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

        cap.start_session("s2", &path, system_spec())
            .await
            .unwrap();
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

        cap.start_session("s3", &path, mixed_spec())
            .await
            .unwrap();
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
        assert!(
            !f.contains("anlms"),
            "unexpected anlms in non-AEC filter: {f}"
        );
    }

    #[test]
    fn mix_filter_with_aec_contains_anlms() {
        let f = build_mix_filter(true);
        assert!(f.contains("anlms=order=512"), "missing anlms: {f}");
    }

    #[test]
    fn mix_filter_with_aec_still_contains_highpass_and_dynaudnorm() {
        let f = build_mix_filter(true);
        assert!(
            f.contains("highpass=f=80"),
            "missing highpass in AEC filter: {f}"
        );
        assert!(
            f.contains("dynaudnorm"),
            "missing dynaudnorm in AEC filter: {f}"
        );
    }

    // ── build_mix_filter_resync — pure function (independent-clock mix) ──────
    // macOS (SCK+cpal) and Windows (WASAPI loopback + cpal mic) both mix two
    // independent-clock legs through this filter, so it is exercised on both.

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn resync_mix_filter_resamples_both_inputs_to_pts() {
        for aec in [false, true] {
            let f = build_mix_filter_resync(aec);
            // Both the mic ([0:a]) and system ([1:a]) inputs must be
            // resampled-to-PTS to absorb the independent-clock drift.
            assert_eq!(
                f.matches("aresample=async=1").count(),
                2,
                "both inputs must be drift-compensated: {f}"
            );
            // And normalised to a common rate/layout so amix/anlms get matching
            // inputs (a mono mic + stereo loopback would otherwise fail to mix).
            assert_eq!(
                f.matches("channel_layouts=stereo").count(),
                2,
                "both inputs must be normalised to stereo: {f}"
            );
            assert!(f.contains("highpass=f=80"), "missing mic cleanup: {f}");
            assert!(f.contains("dynaudnorm"), "missing mic normaliser: {f}");
            assert!(
                f.contains("amix=inputs=2:duration=longest"),
                "missing final mix: {f}"
            );
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn resync_mix_filter_aec_splits_system_pad_for_reuse() {
        let f = build_mix_filter_resync(true);
        assert!(
            f.contains("anlms=order=512"),
            "AEC must subtract system: {f}"
        );
        // A filter-graph pad cannot be consumed twice; the system track is
        // needed by both anlms and amix, so it must be asplit.
        assert!(
            f.contains("asplit=2"),
            "system pad must be split for reuse: {f}"
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn resync_mix_filter_no_aec_has_no_anlms_or_split() {
        let f = build_mix_filter_resync(false);
        assert!(!f.contains("anlms"), "non-AEC must not echo-cancel: {f}");
        assert!(!f.contains("asplit"), "non-AEC needs no pad split: {f}");
    }

    // ── ffmpeg discovery — bundled-next-to-sidecar wins over PATH ─────────────
    // Packaged apps ship ffmpeg beside the sidecar so mixed recording is
    // self-contained; an empty dir falls back to PATH.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[test]
    fn resolve_ffmpeg_prefers_bundled_then_path() {
        use std::ffi::OsStr;
        let dir = tempfile::tempdir().unwrap();
        // No bundled binary → fall back to PATH ("ffmpeg").
        assert_eq!(resolve_ffmpeg_in(dir.path()), OsStr::new("ffmpeg"));
        // A bundled ffmpeg(.exe) next to the sidecar is preferred.
        let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        let bundled = dir.path().join(name);
        std::fs::write(&bundled, b"").unwrap();
        assert_eq!(resolve_ffmpeg_in(dir.path()), bundled.into_os_string());
    }

    // ── 60-min mic↔system clock-drift spike (section-05) ────────────────────
    //
    // Captures macOS system audio (ScreenCaptureKit) and the mic (cpal)
    // simultaneously, then measures how far the two independent audio clocks
    // drift apart. Silence is fine — both backends emit continuous frames, so
    // sample counts reveal the drift without any audio playing.
    //
    // Run the real spike (needs Screen-Recording permission granted):
    //   MA_DRIFT_SPIKE_SECS=3600 cargo test --manifest-path rust/Cargo.toml \
    //     -p meeting-adapters drift_spike_system_vs_mic -- --ignored --nocapture
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "60-min hardware spike; needs Screen-Recording permission"]
    async fn drift_spike_system_vs_mic() {
        use std::time::{Duration, Instant};
        use tempfile::tempdir;

        let secs: u64 = std::env::var("MA_DRIFT_SPIKE_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);

        let dir = tempdir().unwrap();
        let sys_path = dir.path().join("sys.wav");
        let mic_path = dir.path().join("mic.wav");
        let cap = CpalAudioCapture::new();

        cap.start_session("sys", &sys_path, system_spec())
            .await
            .expect("system capture failed to start (Screen-Recording permission?)");
        cap.start_session("mic", &mic_path, mic_spec())
            .await
            .expect("mic capture failed to start");

        // Steady-state window: start/stop transients add a near-equal offset
        // to both streams and largely cancel in the relative metric.
        let t0 = Instant::now();
        tokio::time::sleep(Duration::from_secs(secs)).await;
        let wall = t0.elapsed().as_secs_f64();

        cap.stop_session("sys").await.expect("system stop failed");
        cap.stop_session("mic").await.expect("mic stop failed");

        let (sys_frames, sys_rate) = wav_frames_and_rate(&sys_path);
        let (mic_frames, mic_rate) = wav_frames_and_rate(&mic_path);

        let sys_secs = sys_frames as f64 / sys_rate as f64;
        let mic_secs = mic_frames as f64 / mic_rate as f64;
        let drift_ms = (sys_secs - mic_secs) * 1000.0;
        let drift_ppm = if wall > 0.0 {
            drift_ms / 1000.0 / wall * 1e6
        } else {
            0.0
        };
        let proj_60min_ms = if wall > 0.0 {
            drift_ms / wall * 3600.0
        } else {
            0.0
        };

        println!("──── macOS mic↔system clock-drift spike ────");
        println!("wall window:        {wall:.3} s");
        println!("system (SCK):       {sys_frames} frames @ {sys_rate} Hz = {sys_secs:.3} s");
        println!("mic (cpal):         {mic_frames} frames @ {mic_rate} Hz = {mic_secs:.3} s");
        println!("drift (sys - mic):  {drift_ms:.1} ms  ({drift_ppm:.1} ppm)");
        println!("projected over 60m: {proj_60min_ms:.1} ms");
        println!("────────────────────────────────────────────");

        assert!(sys_frames > 0, "system WAV captured no audio (TCC denied?)");
        assert!(mic_frames > 0, "mic WAV captured no audio");
    }

    #[cfg(target_os = "macos")]
    fn wav_frames_and_rate(path: &std::path::Path) -> (u64, u32) {
        let r = hound::WavReader::open(path)
            .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
        let spec = r.spec();
        // `duration()` is samples *per channel* = frames.
        (u64::from(r.duration()), spec.sample_rate)
    }
}
