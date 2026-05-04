use std::path::PathBuf;
use std::sync::Arc;
use meeting_core::{ports::Transcriber, usecases::transcribe_audio_file};
use meeting_adapters::WhisperTranscriber;
use tauri::State;

struct AppState {
    transcriber: Arc<dyn Transcriber>,
}

#[tauri::command]
async fn transcribe_file(
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let audio_path = PathBuf::from(&path);
    transcribe_audio_file(Arc::clone(&state.transcriber), &audio_path)
        .await
        .map(|t| t.text)
        .map_err(|e| e.to_string())
}

fn default_model_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("$HOME is not set");
            PathBuf::from(home).join(".local/share")
        });
    base.join("meeting-assistant/models/ggml-medium.bin")
}

pub fn run() {
    let model_path = std::env::var_os("MEETING_ASSISTANT_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(default_model_path);

    let transcriber = WhisperTranscriber::new(&model_path)
        .expect("failed to load whisper model — set MEETING_ASSISTANT_MODEL to a valid path");

    tauri::Builder::default()
        .manage(AppState {
            transcriber: Arc::new(transcriber),
        })
        .invoke_handler(tauri::generate_handler![transcribe_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
