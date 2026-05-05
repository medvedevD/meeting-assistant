use axum::{routing::{get, post}, Router};
use std::sync::Arc;
use meeting_core::ports::{JobRepo, LlmProvider, MeetingRepo, TemplateLoader, Transcriber};
use crate::routes::{transcribe, jobs, protocols};

pub struct AppState {
    pub transcriber: Arc<dyn Transcriber>,
    pub meeting_repo: Arc<dyn MeetingRepo>,
    pub job_repo: Arc<dyn JobRepo>,
    pub llm: Arc<dyn LlmProvider>,
    pub templates: Arc<dyn TemplateLoader>,
}

pub fn create_router(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/api/v1/transcribe", post(transcribe::handle))
        .route("/api/v1/jobs", post(jobs::submit))
        .route("/api/v1/jobs/:id", get(jobs::status))
        .route("/api/v1/protocols", post(protocols::generate))
        .with_state(state)
}
