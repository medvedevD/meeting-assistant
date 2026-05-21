use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use std::sync::Arc;
use meeting_core::usecases::list_meetings;
use crate::router::AppState;

#[derive(Serialize)]
pub struct MeetingItem {
    pub id: String,
    pub name: String,
    pub audio_path: String,
    pub has_transcript: bool,
    pub created_at: i64,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<MeetingItem>>, (StatusCode, String)> {
    let meetings = list_meetings(Arc::clone(&state.meeting_repo))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items = meetings
        .into_iter()
        .map(|m| MeetingItem {
            has_transcript: m.transcript_text.is_some(),
            id: m.id,
            name: m.name,
            audio_path: m.audio_path.display().to_string(),
            created_at: m.created_at,
        })
        .collect();

    Ok(Json(items))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use std::path::PathBuf;
    use tower::ServiceExt;
    use meeting_core::{
        entities::Meeting,
        fakes::{FakeAudioCapture, FakeLlmProvider, FakeMeetingFileStore, FakeMeetingRepo, FakeJobRepo, FakeTemplateLoader, FakeTranscriber},
        ports::MeetingRepo,
    };
    use crate::router::{AppState, create_router};

    fn make_app(repo: std::sync::Arc<FakeMeetingRepo>) -> axum::Router {
        create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: repo,
            job_repo: FakeJobRepo::new(),
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: PathBuf::from("/tmp"),
        })
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn empty_repo_returns_empty_array() {
        let repo = FakeMeetingRepo::new();
        let app = make_app(repo);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/meetings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json, serde_json::json!([]));
    }

    #[tokio::test]
    async fn returns_saved_meetings() {
        let repo = FakeMeetingRepo::new();
        let m = Meeting::new("Планёрка".to_string(), PathBuf::from("/a.wav"));
        repo.save(&m).await.unwrap();
        let app = make_app(std::sync::Arc::clone(&repo));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/meetings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "Планёрка");
        assert_eq!(arr[0]["has_transcript"], false);
    }

    #[tokio::test]
    async fn has_transcript_true_when_set() {
        let repo = FakeMeetingRepo::new();
        let m = Meeting::new("Встреча".to_string(), PathBuf::from("/b.wav"));
        repo.save(&m).await.unwrap();
        repo.save_transcript(&m.id, "текст").await.unwrap();
        let app = make_app(std::sync::Arc::clone(&repo));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/meetings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = body_json(response).await;
        assert_eq!(json[0]["has_transcript"], true);
    }
}
