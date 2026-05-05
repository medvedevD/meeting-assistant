use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use meeting_core::{
    entities::Job,
    usecases::{get_job_status, submit_transcription_job},
};
use crate::router::AppState;

#[derive(Deserialize)]
pub struct SubmitRequest {
    pub audio_path: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct JobResponse {
    pub id: String,
    pub meeting_id: String,
    pub kind: String,
    pub status: String,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<Job> for JobResponse {
    fn from(j: Job) -> Self {
        Self {
            id: j.id,
            meeting_id: j.meeting_id,
            kind: j.kind.as_str().to_string(),
            status: j.status.as_str().to_string(),
            attempts: j.attempts,
            last_error: j.last_error,
            created_at: j.created_at,
            updated_at: j.updated_at,
        }
    }
}

pub async fn submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitRequest>,
) -> Result<(StatusCode, Json<JobResponse>), (StatusCode, String)> {
    let job = submit_transcription_job(
        Arc::clone(&state.meeting_repo),
        Arc::clone(&state.job_repo),
        PathBuf::from(req.audio_path),
        req.name,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(job.into())))
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<JobResponse>, (StatusCode, String)> {
    let job = get_job_status(Arc::clone(&state.job_repo), &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("job {id} not found")))?;

    Ok(Json(job.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use meeting_core::fakes::{FakeJobRepo, FakeLlmProvider, FakeMeetingRepo, FakeTemplateLoader, FakeTranscriber};
    use crate::router::{AppState, create_router};

    fn make_app() -> axum::Router {
        create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
        })
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn submit_returns_201_with_job_id() {
        let app = make_app();
        let body = serde_json::json!({ "audio_path": "/a.wav", "name": "Планёрка" });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/jobs")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let json = body_json(response).await;
        assert_eq!(json["status"], "pending");
        assert!(!json["id"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn status_returns_404_for_unknown_job() {
        let app = make_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/jobs/no-such-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn submit_then_status_roundtrip() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: Arc::clone(&mr) as Arc<dyn meeting_core::ports::MeetingRepo>,
            job_repo: Arc::clone(&jr) as Arc<dyn meeting_core::ports::JobRepo>,
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
        });

        // submit
        let body = serde_json::json!({ "audio_path": "/b.wav", "name": "1-на-1" });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/jobs")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let job_json = body_json(response).await;
        let job_id = job_json["id"].as_str().unwrap().to_string();

        // status
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/jobs/{job_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let status_json = body_json(response).await;
        assert_eq!(status_json["id"], job_id);
        assert_eq!(status_json["status"], "pending");
    }
}
