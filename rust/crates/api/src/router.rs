use axum::{routing::{get, post}, Router};
use std::path::PathBuf;
use std::sync::Arc;
use meeting_core::ports::{AudioCapture, JobRepo, LlmProvider, MeetingRepo, TemplateLoader, Transcriber};
use crate::routes::{transcribe, jobs, protocols, recordings, meetings};

pub struct AppState {
    pub transcriber: Arc<dyn Transcriber>,
    pub meeting_repo: Arc<dyn MeetingRepo>,
    pub job_repo: Arc<dyn JobRepo>,
    pub llm: Arc<dyn LlmProvider>,
    pub templates: Arc<dyn TemplateLoader>,
    pub audio_capture: Arc<dyn AudioCapture>,
    /// Directory where per-meeting recording subdirs are created.
    pub recordings_dir: PathBuf,
}

pub fn create_router(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/api/v1/transcribe", post(transcribe::handle))
        .route("/api/v1/jobs", post(jobs::submit))
        .route("/api/v1/jobs/:id", get(jobs::status))
        .route("/api/v1/protocols", post(protocols::generate))
        .route("/api/v1/recordings", post(recordings::start))
        .route("/api/v1/recordings/:id/stop", post(recordings::stop))
        .route("/api/v1/meetings", get(meetings::list))
        .with_state(state)
}
