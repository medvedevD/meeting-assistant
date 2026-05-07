use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use tauri_plugin_store::StoreExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use meeting_core::{
    ports::{AudioCapture, CaptureSource, JobRepo, LlmProvider, MeetingRepo, TemplateLoader, Transcriber},
    usecases::{
        generate_protocol, get_job_status, list_meetings,
        start_recording, stop_recording, submit_transcription_job, transcribe_audio_file,
    },
};
use meeting_adapters::{
    AnthropicProvider, CpalAudioCapture, Db,
    FileTemplateLoader, SqliteJobRepo, SqliteMeetingRepo, WhisperTranscriber,
};

// ── Ring log buffer ───────────────────────────────────────────────────────────

type RingLogBuffer = Arc<Mutex<VecDeque<String>>>;

struct RingWriter {
    buffer: RingLogBuffer,
    capacity: usize,
}

impl io::Write for RingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        let line = s.trim_end_matches(['\n', '\r']);
        if !line.is_empty() {
            let mut lock = self.buffer.lock();
            if lock.len() >= self.capacity {
                lock.pop_front();
            }
            lock.push_back(line.to_owned());
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

struct RingMakeWriter {
    buffer: RingLogBuffer,
    capacity: usize,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RingMakeWriter {
    type Writer = RingWriter;
    fn make_writer(&'a self) -> Self::Writer {
        RingWriter { buffer: Arc::clone(&self.buffer), capacity: self.capacity }
    }
}

// ── App state ────────────────────────────────────────────────────────────────

struct AppState {
    transcriber:    Arc<dyn Transcriber>,
    meeting_repo:   Arc<dyn MeetingRepo>,
    audio_capture:  Arc<dyn AudioCapture>,
    job_repo:       Arc<dyn JobRepo>,
    llm:            Arc<dyn LlmProvider>,
    templates:      Arc<dyn TemplateLoader>,
    recordings_dir: PathBuf,
    // stored for settings_get / diagnostics
    model_path:     PathBuf,
    db_path:        PathBuf,
    prompts_dir:    PathBuf,
    log_buffer:     RingLogBuffer,
}

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct MeetingDto {
    pub id: String,
    pub name: String,
    pub audio_path: String,
    pub has_transcript: bool,
    pub has_protocol: bool,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct RecordingDto {
    pub id: String,
    pub name: String,
    pub audio_path: String,
}

#[derive(Serialize)]
pub struct ProtocolDto {
    pub markdown: String,
}

#[derive(Serialize)]
pub struct JobDto {
    pub id: String,
    pub meeting_id: String,
    pub status: String,
    pub attempts: u32,
    pub last_error: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SettingsPaths {
    pub model:      Option<String>,
    pub db:         Option<String>,
    pub recordings: Option<String>,
    pub prompts:    Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct RecordingPrefs {
    pub source:      String,
    pub echo_cancel: bool,
}

#[derive(Serialize, Deserialize)]
pub struct SettingsDto {
    pub paths:            SettingsPaths,
    pub anthropic_api_key: Option<String>,
    pub recording:        RecordingPrefs,
    pub default_template: Option<String>,
}

#[derive(Serialize)]
pub struct PathInfoDto {
    pub path:       String,
    pub exists:     bool,
    pub writable:   bool,
    pub size_bytes: Option<u64>,
}

#[derive(Serialize)]
pub struct DeviceInfoDto {
    pub name:       String,
    pub is_default: bool,
}

#[derive(Serialize)]
pub struct DiagnosticsPathsDto {
    pub model:      PathInfoDto,
    pub db:         PathInfoDto,
    pub recordings: PathInfoDto,
    pub prompts:    PathInfoDto,
}

#[derive(Serialize)]
pub struct DiagnosticsDto {
    pub os:                &'static str,
    pub arch:              &'static str,
    pub app_version:       &'static str,
    pub cpal_host:         String,
    pub input_devices:     Vec<DeviceInfoDto>,
    pub output_devices:    Vec<DeviceInfoDto>,
    pub paths:             DiagnosticsPathsDto,
    pub has_anthropic_key: bool,
    pub ffmpeg_ok:         bool,
    pub logs:              Vec<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn job_to_dto(j: meeting_core::entities::Job) -> JobDto {
    JobDto {
        id:         j.id,
        meeting_id: j.meeting_id,
        status:     j.status.as_str().to_string(),
        attempts:   j.attempts,
        last_error: j.last_error,
    }
}

fn path_info(p: &Path) -> PathInfoDto {
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
    PathInfoDto {
        path: p.display().to_string(),
        exists,
        writable,
        size_bytes,
    }
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
async fn transcribe_file(
    path:  String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    transcribe_audio_file(Arc::clone(&state.transcriber), &PathBuf::from(path))
        .await
        .map(|t| t.text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn recording_start(
    name:        Option<String>,
    source:      Option<String>,
    echo_cancel: Option<bool>,
    state:       State<'_, AppState>,
) -> Result<RecordingDto, String> {
    let capture_source = match source.as_deref() {
        Some("system") => CaptureSource::System,
        Some("mixed")  => CaptureSource::Mixed,
        _              => CaptureSource::Mic,
    };
    let meeting = start_recording(
        Arc::clone(&state.audio_capture),
        Arc::clone(&state.meeting_repo),
        &state.recordings_dir,
        name,
        capture_source,
        echo_cancel.unwrap_or(false),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(RecordingDto {
        id:         meeting.id,
        name:       meeting.name,
        audio_path: meeting.audio_path.display().to_string(),
    })
}

#[tauri::command]
async fn recording_stop(
    id:    String,
    state: State<'_, AppState>,
) -> Result<RecordingDto, String> {
    let meeting = stop_recording(
        Arc::clone(&state.audio_capture),
        Arc::clone(&state.meeting_repo),
        &id,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(RecordingDto {
        id:         meeting.id,
        name:       meeting.name,
        audio_path: meeting.audio_path.display().to_string(),
    })
}

#[tauri::command]
async fn meetings_list(
    state: State<'_, AppState>,
) -> Result<Vec<MeetingDto>, String> {
    let meetings = list_meetings(Arc::clone(&state.meeting_repo))
        .await
        .map_err(|e| e.to_string())?;

    Ok(meetings
        .into_iter()
        .map(|m| MeetingDto {
            has_transcript: m.transcript_text.is_some(),
            has_protocol:   m.protocol_text.is_some(),
            id:         m.id,
            name:       m.name,
            audio_path: m.audio_path.display().to_string(),
            created_at: m.created_at,
        })
        .collect())
}

#[tauri::command]
async fn protocol_generate(
    meeting_id:    String,
    template_name: Option<String>,
    state:         State<'_, AppState>,
) -> Result<ProtocolDto, String> {
    let meeting = state.meeting_repo
        .find_by_id(&meeting_id).await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("meeting '{meeting_id}' not found"))?;

    let transcript = meeting.transcript_text
        .ok_or_else(|| "meeting has no transcript yet".to_string())?;

    let protocol = generate_protocol(
        Arc::clone(&state.llm),
        Arc::clone(&state.templates),
        &transcript,
        template_name.as_deref(),
        Some(&meeting.name),
    )
    .await
    .map_err(|e| e.to_string())?;

    state.meeting_repo
        .save_protocol(&meeting_id, &protocol.markdown)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ProtocolDto { markdown: protocol.markdown })
}

#[tauri::command]
async fn protocol_load(
    meeting_id: String,
    state: State<'_, AppState>,
) -> Result<Option<ProtocolDto>, String> {
    let meeting = state.meeting_repo
        .find_by_id(&meeting_id).await
        .map_err(|e| e.to_string())?;

    Ok(meeting.and_then(|m| m.protocol_text.map(|md| ProtocolDto { markdown: md })))
}

#[tauri::command]
async fn templates_list(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    state.templates.list_names().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn job_submit(
    path:  String,
    name:  Option<String>,
    state: State<'_, AppState>,
) -> Result<JobDto, String> {
    let job_name = name.unwrap_or_else(|| "Untitled meeting".to_string());
    let job = submit_transcription_job(
        Arc::clone(&state.meeting_repo),
        Arc::clone(&state.job_repo),
        PathBuf::from(path),
        job_name,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(job_to_dto(job))
}

#[tauri::command]
async fn job_status(
    id:    String,
    state: State<'_, AppState>,
) -> Result<Option<JobDto>, String> {
    let job = get_job_status(Arc::clone(&state.job_repo), &id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(job.map(job_to_dto))
}

#[tauri::command]
async fn settings_get(
    state: State<'_, AppState>,
    app:   tauri::AppHandle,
) -> Result<SettingsDto, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;

    // Effective paths: store value ?? state (which already applied env ?? default at startup)
    let paths = SettingsPaths {
        model:      store_str(&store, "paths.model")
                        .or_else(|| Some(state.model_path.display().to_string())),
        db:         store_str(&store, "paths.db")
                        .or_else(|| Some(state.db_path.display().to_string())),
        recordings: store_str(&store, "paths.recordings")
                        .or_else(|| Some(state.recordings_dir.display().to_string())),
        prompts:    store_str(&store, "paths.prompts")
                        .or_else(|| Some(state.prompts_dir.display().to_string())),
    };

    let recording = RecordingPrefs {
        source:      store_str(&store, "recording.source").unwrap_or_else(|| "mic".to_string()),
        echo_cancel: store.get("recording.echo_cancel")
                         .and_then(|v| v.as_bool())
                         .unwrap_or(false),
    };

    Ok(SettingsDto {
        paths,
        anthropic_api_key: None, // never expose the key
        recording,
        default_template: store_str(&store, "default_template"),
    })
}

#[tauri::command]
async fn settings_set(
    dto: SettingsDto,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;

    set_or_delete(&store, "paths.model",      dto.paths.model.as_deref());
    set_or_delete(&store, "paths.db",         dto.paths.db.as_deref());
    set_or_delete(&store, "paths.recordings", dto.paths.recordings.as_deref());
    set_or_delete(&store, "paths.prompts",    dto.paths.prompts.as_deref());
    // API key: None means "don't touch" (we never send the key back from settings_get).
    // Some("") means "clear". Some("sk-...") means "set new value".
    if let Some(key) = dto.anthropic_api_key {
        if key.trim().is_empty() {
            let _ = store.delete("anthropic_api_key");
        } else {
            store.set("anthropic_api_key", serde_json::json!(key));
        }
    }
    store.set("recording.source",      serde_json::json!(dto.recording.source));
    store.set("recording.echo_cancel", serde_json::json!(dto.recording.echo_cancel));
    set_or_delete(&store, "default_template", dto.default_template.as_deref());

    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
async fn diagnostics(
    state: State<'_, AppState>,
) -> Result<DiagnosticsDto, String> {
    struct AudioInfo {
        cpal_host: String,
        input_devices: Vec<DeviceInfoDto>,
        output_devices: Vec<DeviceInfoDto>,
    }

    // CPAL device enumeration blocks on Linux (PipeWire queries) — must not run on async thread.
    let audio = tokio::task::spawn_blocking(|| {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let cpal_host = host.id().name().to_string();
        let default_in_name = host.default_input_device().and_then(|d| d.name().ok());
        let default_out_name = host.default_output_device().and_then(|d| d.name().ok());
        let input_devices = host.input_devices()
            .map(|iter| {
                iter.filter_map(|d| d.name().ok())
                    .map(|name| {
                        let is_default = default_in_name.as_deref() == Some(&name);
                        DeviceInfoDto { name, is_default }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let output_devices = host.output_devices()
            .map(|iter| {
                iter.filter_map(|d| d.name().ok())
                    .map(|name| {
                        let is_default = default_out_name.as_deref() == Some(&name);
                        DeviceInfoDto { name, is_default }
                    })
                    .collect()
            })
            .unwrap_or_default();
        AudioInfo { cpal_host, input_devices, output_devices }
    })
    .await
    .map_err(|e| e.to_string())?;

    let ffmpeg_ok = tokio::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    let logs = {
        let lock = state.log_buffer.lock();
        lock.iter().cloned().collect()
    };

    Ok(DiagnosticsDto {
        os:   std::env::consts::OS,
        arch: std::env::consts::ARCH,
        app_version: env!("CARGO_PKG_VERSION"),
        cpal_host: audio.cpal_host,
        input_devices: audio.input_devices,
        output_devices: audio.output_devices,
        paths: DiagnosticsPathsDto {
            model:      path_info(&state.model_path),
            db:         path_info(&state.db_path),
            recordings: path_info(&state.recordings_dir),
            prompts:    path_info(&state.prompts_dir),
        },
        has_anthropic_key: std::env::var("ANTHROPIC_API_KEY").is_ok(),
        ffmpeg_ok,
        logs,
    })
}

#[tauri::command]
async fn diagnostics_logs(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let lock = state.log_buffer.lock();
    Ok(lock.iter().cloned().collect())
}

#[tauri::command]
async fn show_debug_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("debug") {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn open_path(path: String) -> Result<(), String> {
    let cmd = if cfg!(target_os = "linux") { "xdg-open" }
              else if cfg!(target_os = "macos") { "open" }
              else { "explorer" };
    tokio::process::Command::new(cmd)
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Store helpers ─────────────────────────────────────────────────────────────

fn store_str(store: &tauri_plugin_store::Store<tauri::Wry>, key: &str) -> Option<String> {
    store.get(key)?.as_str().map(str::to_owned)
}

fn set_or_delete(store: &tauri_plugin_store::Store<tauri::Wry>, key: &str, value: Option<&str>) {
    match value {
        Some(v) if !v.trim().is_empty() => store.set(key, serde_json::json!(v)),
        _ => { let _ = store.delete(key); }
    }
}

// ── Startup settings (read store before app build) ────────────────────────────

struct StartupSettings {
    model_path:     Option<PathBuf>,
    db_path:        Option<PathBuf>,
    recordings_dir: Option<PathBuf>,
    prompts_dir:    Option<PathBuf>,
    anthropic_key:  Option<String>,
}

fn read_startup_settings() -> StartupSettings {
    // tauri-plugin-store writes to {app_data_dir}/{store_name}
    // app_data_dir on Linux = $XDG_DATA_HOME/{bundle_id} or ~/.local/share/{bundle_id}
    let store_path = xdg_data_dir()
        .join("dev.codemedvedev.meeting-assistant/settings.json");

    let content = match std::fs::read_to_string(&store_path) {
        Ok(c) => c,
        Err(_) => return StartupSettings { model_path: None, db_path: None,
                       recordings_dir: None, prompts_dir: None, anthropic_key: None },
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return StartupSettings { model_path: None, db_path: None,
                       recordings_dir: None, prompts_dir: None, anthropic_key: None },
    };

    let str_val = |key: &str| -> Option<PathBuf> {
        json.get(key)?.as_str().filter(|s| !s.trim().is_empty()).map(PathBuf::from)
    };

    StartupSettings {
        model_path:     str_val("paths.model"),
        db_path:        str_val("paths.db"),
        recordings_dir: str_val("paths.recordings"),
        prompts_dir:    str_val("paths.prompts"),
        anthropic_key:  json.get("anthropic_api_key")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.trim().is_empty())
                            .map(str::to_owned),
    }
}

// ── Bootstrap ────────────────────────────────────────────────────────────────

pub fn run() {
    let log_buffer: RingLogBuffer = Arc::new(Mutex::new(VecDeque::with_capacity(500)));
    let ring_writer = RingMakeWriter { buffer: Arc::clone(&log_buffer), capacity: 500 };

    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::fmt::layer()
            .with_writer(ring_writer)
            .with_ansi(false))
        .try_init();

    let startup = read_startup_settings();

    // If API key is stored, set it in env before building providers
    if let Some(key) = &startup.anthropic_key {
        // SAFETY: single-threaded at this point, no other threads reading env
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", key); }
    }

    let model_path = startup.model_path
        .or_else(|| std::env::var_os("MEETING_ASSISTANT_MODEL").map(PathBuf::from))
        .unwrap_or_else(|| xdg_data_dir().join("meeting-assistant/models/ggml-medium.bin"));

    let db_path = startup.db_path
        .or_else(|| std::env::var_os("MEETING_ASSISTANT_DB").map(PathBuf::from))
        .unwrap_or_else(|| xdg_data_dir().join("meeting-assistant/rust-index.db"));

    let recordings_dir = startup.recordings_dir
        .or_else(|| std::env::var_os("MEETING_ASSISTANT_RECORDINGS").map(PathBuf::from))
        .unwrap_or_else(|| xdg_cache_dir().join("meeting-assistant/recordings"));

    let prompts_dir = startup.prompts_dir
        .or_else(|| std::env::var_os("MEETING_ASSISTANT_PROMPTS").map(PathBuf::from))
        .unwrap_or_else(default_prompts_dir);

    let transcriber = WhisperTranscriber::new(&model_path)
        .expect("failed to load whisper model — set MEETING_ASSISTANT_MODEL to a valid path");

    let db = Db::open(&db_path).expect("failed to open database");
    let meeting_repo: Arc<dyn MeetingRepo> = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
    let job_repo:     Arc<dyn JobRepo>     = Arc::new(SqliteJobRepo(Arc::clone(&db)));

    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    let llm:       Arc<dyn LlmProvider>    = Arc::new(AnthropicProvider::new(api_key));
    let templates: Arc<dyn TemplateLoader> = Arc::new(FileTemplateLoader::new(&prompts_dir));
    let audio_capture: Arc<dyn AudioCapture> = Arc::new(CpalAudioCapture::new());
    let transcriber: Arc<dyn Transcriber>   = Arc::new(transcriber);

    let worker = meeting_adapters::Worker::new(
        Arc::clone(&job_repo),
        Arc::clone(&meeting_repo),
        Arc::clone(&transcriber),
    );

    let state = AppState {
        transcriber,
        meeting_repo,
        audio_capture,
        job_repo,
        llm,
        templates,
        recordings_dir,
        model_path,
        db_path,
        prompts_dir,
        log_buffer,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|_app| {
            tauri::async_runtime::spawn(worker.run());
            Ok(())
        })
        .manage(state)
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Destroyed if window.label() == "main" => {
                    std::process::exit(0);
                }
                tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "debug" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            transcribe_file,
            recording_start,
            recording_stop,
            meetings_list,
            protocol_generate,
            protocol_load,
            templates_list,
            job_submit,
            job_status,
            settings_get,
            settings_set,
            diagnostics,
            diagnostics_logs,
            open_path,
            show_debug_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn default_prompts_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.ancestors().nth(4).map(|root| root.join("prompts")))
        .unwrap_or_else(|| PathBuf::from("prompts"))
}

fn xdg_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join(".local/share"))
}

fn xdg_cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cache"))
}
