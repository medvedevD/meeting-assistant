use std::path::PathBuf;
use std::sync::Arc;
use serde::Serialize;
use tauri::State;
use meeting_core::{
    ports::{AudioCapture, CaptureSource, MeetingRepo, Transcriber},
    usecases::{list_meetings, start_recording, stop_recording, transcribe_audio_file},
};
use meeting_adapters::{CpalAudioCapture, Db, SqliteMeetingRepo, WhisperTranscriber};

// ── App state ────────────────────────────────────────────────────────────────

struct AppState {
    transcriber: Arc<dyn Transcriber>,
    meeting_repo: Arc<dyn MeetingRepo>,
    audio_capture: Arc<dyn AudioCapture>,
    recordings_dir: PathBuf,
}

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct MeetingDto {
    pub id: String,
    pub name: String,
    pub audio_path: String,
    pub has_transcript: bool,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct RecordingDto {
    pub id: String,
    pub name: String,
    pub audio_path: String,
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
async fn transcribe_file(
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    transcribe_audio_file(Arc::clone(&state.transcriber), &PathBuf::from(path))
        .await
        .map(|t| t.text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn recording_start(
    name: Option<String>,
    source: Option<String>,
    echo_cancel: Option<bool>,
    state: State<'_, AppState>,
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
        id: meeting.id,
        name: meeting.name,
        audio_path: meeting.audio_path.display().to_string(),
    })
}

#[tauri::command]
async fn recording_stop(
    id: String,
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
        id: meeting.id,
        name: meeting.name,
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
            id: m.id,
            name: m.name,
            audio_path: m.audio_path.display().to_string(),
            created_at: m.created_at,
        })
        .collect())
}

// ── Bootstrap ────────────────────────────────────────────────────────────────

pub fn run() {
    let model_path = std::env::var_os("MEETING_ASSISTANT_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg_data_dir().join("meeting-assistant/models/ggml-medium.bin"));

    let db_path = std::env::var_os("MEETING_ASSISTANT_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg_data_dir().join("meeting-assistant/rust-index.db"));

    let recordings_dir = std::env::var_os("MEETING_ASSISTANT_RECORDINGS")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg_cache_dir().join("meeting-assistant/recordings"));

    let transcriber = WhisperTranscriber::new(&model_path)
        .expect("failed to load whisper model — set MEETING_ASSISTANT_MODEL to a valid path");

    let db = Db::open(&db_path).expect("failed to open database");
    let meeting_repo = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
    let audio_capture = Arc::new(CpalAudioCapture::new());

    tauri::Builder::default()
        .manage(AppState {
            transcriber: Arc::new(transcriber),
            meeting_repo,
            audio_capture,
            recordings_dir,
        })
        .invoke_handler(tauri::generate_handler![
            transcribe_file,
            recording_start,
            recording_stop,
            meetings_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
