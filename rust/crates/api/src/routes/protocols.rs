use crate::router::AppState;
use axum::{extract::State, http::StatusCode, Json};
use meeting_core::usecases::generate_protocol;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    // Decision #3: when the request omits a template, resolve the configured
    // `default_template` here (API layer) so the use-case stays settings-free.
    let template_name = req
        .template_name
        .clone()
        .or_else(|| (state.default_template)());
    let protocol = generate_protocol(
        Arc::clone(&state.llm),
        Arc::clone(&state.templates),
        &req.transcript,
        template_name.as_deref(),
        req.meeting_name.as_deref(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::OK,
        Json(GenerateResponse {
            markdown: protocol.markdown,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use crate::router::{create_router, AppState};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use meeting_core::fakes::{
        FakeAudioCapture, FakeJobRepo, FakeLlmProvider, FakeMeetingFileStore, FakeMeetingRepo,
        FakeTemplateLoader, FakeTranscriber,
    };
    use tower::ServiceExt;

    fn make_app() -> axum::Router {
        create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new("# Протокол\n\nТекст."),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
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
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
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
        let templates = FakeTemplateLoader::new([(
            "1-на-1",
            "Протокол встречи {meeting_name}.\n{transcript}\n",
        )]);

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new("# 1-на-1 протокол"),
            templates,
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
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

    #[tokio::test]
    async fn resolves_default_template_when_request_omits_one() {
        // Decision #3: with no `template_name` in the request, the API layer
        // falls back to the configured default. The FakeTemplateLoader only
        // knows "Ретро"; if the default weren't resolved, the use-case would
        // hit the built-in prompt and the loader would never be consulted —
        // so a 200 here proves the named template was selected.
        let templates = FakeTemplateLoader::new([("Ретро", "Ретро-протокол.\n{transcript}\n")]);

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new("# Ретро протокол"),
            templates,
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: std::sync::Arc::new(|| Some("Ретро".to_string())),
        });

        // No template_name in the body.
        let body = serde_json::json!({ "transcript": "Обсудили спринт." });

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
        assert_eq!(json["markdown"], "# Ретро протокол");
    }

    #[tokio::test]
    async fn explicit_template_overrides_default() {
        // An explicit `template_name` must win over the configured default.
        let templates = FakeTemplateLoader::new([
            ("1-на-1", "1-на-1.\n{transcript}\n"),
            ("Дефолт", "Дефолт.\n{transcript}\n"),
        ]);

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new("# 1-на-1"),
            templates,
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: std::sync::Arc::new(|| Some("Дефолт".to_string())),
        });

        let body = serde_json::json!({
            "transcript": "текст",
            "template_name": "1-на-1"
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

        // 200 with the 1-на-1 body proves the explicit name was used, not "Дефолт".
        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["markdown"], "# 1-на-1");
    }
}
