use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use meeting_core::usecases::generate_protocol;
use crate::router::AppState;

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub transcript: String,
    pub template_name: Option<String>,
    pub meeting_name: Option<String>,
}

#[derive(Serialize)]
pub struct GenerateResponse {
    pub markdown: String,
}

pub async fn generate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GenerateRequest>,
) -> Result<(StatusCode, Json<GenerateResponse>), (StatusCode, String)> {
    let protocol = generate_protocol(
        Arc::clone(&state.llm),
        Arc::clone(&state.templates),
        &req.transcript,
        req.template_name.as_deref(),
        req.meeting_name.as_deref(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::OK, Json(GenerateResponse { markdown: protocol.markdown })))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use meeting_core::fakes::{FakeAudioCapture, FakeLlmProvider, FakeMeetingRepo, FakeJobRepo, FakeTemplateLoader, FakeTranscriber};
    use crate::router::{AppState, create_router};

    fn make_app() -> axum::Router {
        create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new("# Протокол\n\nТекст."),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
        })
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn returns_200_with_markdown() {
        let app = make_app();
        let body = serde_json::json!({ "transcript": "Обсудили задачи." });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/protocols")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["markdown"], "# Протокол\n\nТекст.");
    }

    #[tokio::test]
    async fn returns_500_when_template_not_found() {
        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new("irrelevant"),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
        });

        let body = serde_json::json!({
            "transcript": "text",
            "template_name": "НесуществующийШаблон"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/protocols")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn uses_template_when_provided() {
        let templates = FakeTemplateLoader::new([
            ("1-на-1", "Протокол встречи {meeting_name}.\n{transcript}\n"),
        ]);

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new("# 1-на-1 протокол"),
            templates,
            audio_capture: FakeAudioCapture::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
        });

        let body = serde_json::json!({
            "transcript": "Обсудили планы.",
            "template_name": "1-на-1",
            "meeting_name": "Дион"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/protocols")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["markdown"], "# 1-на-1 протокол");
    }
}
