use crate::router::AppState;
use axum::{extract::State, http::StatusCode, Json};
use meeting_core::usecases::transcribe_audio_file;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct TranscribeRequest {
    pub path: String,
    /// If provided, saves the transcript text to this meeting record in the DB.
    pub meeting_id: Option<String>,
}

#[derive(Serialize)]
pub struct TranscribeResponse {
    pub text: String,
    pub language: String,
    pub segments: Vec<SegmentDto>,
}

#[derive(Serialize)]
pub struct SegmentDto {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

pub async fn handle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TranscribeRequest>,
) -> Result<Json<TranscribeResponse>, (StatusCode, String)> {
    let path = PathBuf::from(&req.path);
    let transcript = transcribe_audio_file(Arc::clone(&state.transcriber), &path)
        .await
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;

    if let Some(ref id) = req.meeting_id {
        // Mirror the worker's persistence policy: write `transcript.md` next to
        // the audio and record the path; fall back to text-only if the file
        // write fails so the transcript is never lost.
        let meeting = state
            .meeting_repo
            .find_by_id(id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let saved_to_file = if let Some(m) = meeting.as_ref() {
            match state
                .file_store
                .write_transcript(&m.meeting_dir, &transcript.text)
                .await
            {
                Ok(file_path) => {
                    state
                        .meeting_repo
                        .save_transcript_file(id, &transcript.text, &file_path)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    true
                }
                Err(e) => {
                    tracing::warn!(meeting_id = %id, "write transcript.md failed: {e}");
                    false
                }
            }
        } else {
            false
        };

        if !saved_to_file {
            state
                .meeting_repo
                .save_transcript(id, &transcript.text)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }

    Ok(Json(TranscribeResponse {
        text: transcript.text,
        language: transcript.language,
        segments: transcript
            .segments
            .into_iter()
            .map(|s| SegmentDto {
                start_ms: s.start_ms,
                end_ms: s.end_ms,
                text: s.text,
            })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use crate::router::{create_router, AppState};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use meeting_core::{
        entities::Meeting,
        fakes::{
            FakeAudioCapture, FakeJobRepo, FakeLlmProvider, FakeMeetingFileStore, FakeMeetingRepo,
            FakeTemplateLoader, FakeTranscriber,
        },
        ports::MeetingRepo,
    };
    use std::path::PathBuf;
    use tower::ServiceExt;

    #[tokio::test]
    async fn writes_transcript_md_next_to_audio_and_records_path() {
        // `transcribe_audio_file` rejects with 422 if the audio file is missing,
        // so create a real tempfile to simulate the recording on disk.
        let meeting_dir = std::env::temp_dir().join(format!(
            "meeting-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&meeting_dir).unwrap();
        let audio_path = meeting_dir.join("recording.wav");
        std::fs::write(&audio_path, b"dummy").unwrap();

        let mut meeting = Meeting::new("Встреча".to_string(), audio_path.clone());
        meeting.meeting_dir = meeting_dir.clone();
        let meeting_id = meeting.id.clone();

        let repo = FakeMeetingRepo::new();
        repo.save(&meeting).await.unwrap();
        let file_store = FakeMeetingFileStore::new();

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("распознанный текст"),
            meeting_repo: std::sync::Arc::clone(&repo) as _,
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: std::sync::Arc::clone(&file_store) as _,
            recordings_dir: PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
        });

        let body = serde_json::json!({
            "path": audio_path.display().to_string(),
            "meeting_id": meeting_id,
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/transcribe")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let written = file_store.written.lock().unwrap().clone();
        assert_eq!(written.len(), 1, "expected exactly one file write");
        assert_eq!(written[0].0, meeting_dir.join("transcript.md"));
        assert_eq!(written[0].1, "распознанный текст");

        let saved = repo.find_by_id(&meeting_id).await.unwrap().unwrap();
        assert_eq!(saved.transcript_text.as_deref(), Some("распознанный текст"));
        assert_eq!(
            saved.transcript_path,
            Some(meeting_dir.join("transcript.md"))
        );

        let _ = std::fs::remove_dir_all(&meeting_dir);
    }
}
