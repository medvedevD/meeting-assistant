use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use meeting_core::usecases::transcribe_audio_file;
use crate::router::AppState;

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
        state.meeting_repo
            .save_transcript(id, &transcript.text)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(Json(TranscribeResponse {
        text: transcript.text,
        language: transcript.language,
        segments: transcript.segments
            .into_iter()
            .map(|s| SegmentDto { start_ms: s.start_ms, end_ms: s.end_ms, text: s.text })
            .collect(),
    }))
}
