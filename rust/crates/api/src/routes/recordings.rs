use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use meeting_core::usecases::{start_recording, stop_recording};
use crate::router::AppState;

#[derive(Deserialize)]
pub struct StartRequest {
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct MeetingResponse {
    pub id: String,
    pub name: String,
    pub audio_path: String,
    pub created_at: i64,
}

pub async fn start(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartRequest>,
) -> Result<(StatusCode, Json<MeetingResponse>), (StatusCode, String)> {
    let meeting = start_recording(
        Arc::clone(&state.audio_capture),
        Arc::clone(&state.meeting_repo),
        &state.recordings_dir,
        req.name,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(MeetingResponse {
            id: meeting.id,
            name: meeting.name,
            audio_path: meeting.audio_path.display().to_string(),
            created_at: meeting.created_at,
        }),
    ))
}

pub async fn stop(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<MeetingResponse>), (StatusCode, String)> {
    let meeting = stop_recording(
        Arc::clone(&state.audio_capture),
        Arc::clone(&state.meeting_repo),
        &id,
    )
    .await
    .map_err(|e| {
        use meeting_core::CoreError;
        match e {
            CoreError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
            CoreError::Recording(_) => (StatusCode::CONFLICT, e.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        }
    })?;

    Ok((
        StatusCode::OK,
        Json(MeetingResponse {
            id: meeting.id,
            name: meeting.name,
            audio_path: meeting.audio_path.display().to_string(),
            created_at: meeting.created_at,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use std::path::PathBuf;
    use tower::ServiceExt;
    use meeting_core::{
        fakes::{FakeAudioCapture, FakeLlmProvider, FakeMeetingRepo, FakeJobRepo, FakeTemplateLoader, FakeTranscriber},
        ports::AudioCapture,
    };
    use crate::router::{AppState, create_router};

    fn make_app_with(
        capture: std::sync::Arc<FakeAudioCapture>,
        repo: std::sync::Arc<FakeMeetingRepo>,
    ) -> axum::Router {
        create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: repo,
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: capture,
            recordings_dir: PathBuf::from("/tmp/recordings"),
        })
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    // ── start ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn start_returns_201_with_meeting_id() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();
        let app = make_app_with(std::sync::Arc::clone(&capture), std::sync::Arc::clone(&repo));

        let body = serde_json::json!({ "name": "Планёрка" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/recordings")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let json = body_json(response).await;
        assert_eq!(json["name"], "Планёрка");
        assert!(json["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn start_activates_capture_session() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();
        let app = make_app_with(std::sync::Arc::clone(&capture), std::sync::Arc::clone(&repo));

        let body = serde_json::json!({ "name": "1-на-1" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/recordings")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = body_json(response).await;
        let id = json["id"].as_str().unwrap().to_string();
        assert!(capture.is_active(&id));
    }

    #[tokio::test]
    async fn start_without_name_uses_default() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();
        let app = make_app_with(std::sync::Arc::clone(&capture), std::sync::Arc::clone(&repo));

        let body = serde_json::json!({});
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/recordings")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let json = body_json(response).await;
        assert!(json["name"].as_str().map(|s| !s.is_empty()).unwrap_or(false));
    }

    // ── stop ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn stop_returns_200_and_deactivates() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();
        let app = make_app_with(std::sync::Arc::clone(&capture), std::sync::Arc::clone(&repo));

        // Start first
        let start_body = serde_json::json!({ "name": "Дейлик" });
        let start_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/recordings")
                    .header("content-type", "application/json")
                    .body(Body::from(start_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let start_json = body_json(start_resp).await;
        let id = start_json["id"].as_str().unwrap().to_string();

        // Stop
        let stop_resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/recordings/{id}/stop"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(stop_resp.status(), StatusCode::OK);
        assert!(!capture.is_active(&id));
    }

    #[tokio::test]
    async fn stop_unknown_id_returns_conflict() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();
        let app = make_app_with(capture, repo);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/recordings/no-such-id/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
