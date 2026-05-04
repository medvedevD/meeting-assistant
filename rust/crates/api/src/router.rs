use axum::{routing::post, Router};
use std::sync::Arc;
use meeting_core::ports::Transcriber;
use crate::routes::transcribe;

pub struct AppState {
    pub transcriber: Arc<dyn Transcriber>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/transcribe", post(transcribe::handle))
        .with_state(Arc::new(state))
}
