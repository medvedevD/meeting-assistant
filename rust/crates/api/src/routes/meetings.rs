use crate::router::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use meeting_core::{
    usecases::{
        delete_meeting, import_audio, list_meetings, regenerate_protocol, reprocess_transcribe,
        scan_recordings_dir, DeleteMode, ScanCandidate, DEFAULT_MAX_DEPTH,
    },
    CoreError,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

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

// ── Detail (single meeting, incl. persisted protocol) ───────────────────────

#[derive(Serialize)]
pub struct MeetingDetail {
    pub id: String,
    pub name: String,
    pub audio_path: String,
    pub has_transcript: bool,
    pub created_at: i64,
    /// Persisted protocol markdown, when one has been generated. This is the
    /// readback the client uses to render a protocol after a restart (the
    /// in-session client cache no longer stands in for it).
    pub protocol: Option<String>,
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<MeetingDetail>, (StatusCode, String)> {
    let meeting = state
        .meeting_repo
        .find_by_id(&id)
        .await
        .map_err(map_core_err)?
        .ok_or((StatusCode::NOT_FOUND, "meeting not found".into()))?;

    Ok(Json(MeetingDetail {
        has_transcript: meeting.transcript_text.is_some(),
        protocol: meeting.protocol_text.filter(|t| !t.is_empty()),
        id: meeting.id,
        name: meeting.name,
        audio_path: meeting.audio_path.display().to_string(),
        created_at: meeting.created_at,
    }))
}

// ── Import ─────────────────────────────────────────────────────────────────

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
pub struct ImportRequest {
    /// Local filesystem path from the Qt FileDialog / drag&drop URI.
    pub path: String,
    /// Copy the source into `recordings_dir/<id>/` (default). `false` registers
    /// the file in place — used when importing files surfaced by `scan` ("Из папки").
    #[serde(default = "default_true")]
    pub copy: bool,
    /// Queue transcription immediately after import (default).
    #[serde(default = "default_true")]
    pub transcribe: bool,
}

#[derive(Serialize)]
pub struct ImportResponse {
    pub id: String,
    pub name: String,
    pub audio_path: String,
    pub job_id: Option<String>,
}

#[derive(Serialize)]
pub struct ConflictResponse {
    /// Id of the existing meeting that already owns this audio path.
    pub existing_id: String,
    pub error: String,
}

pub async fn import(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImportRequest>,
) -> Result<(StatusCode, Json<ImportResponse>), (StatusCode, Json<ConflictResponse>)> {
    let source = PathBuf::from(&req.path);

    // Dedup by audio_path (decision #10): if this exact path is already a
    // meeting, return 409 pointing at it instead of creating a duplicate.
    if let Ok(Some(existing)) = state.meeting_repo.find_by_audio_path(&source).await {
        return Err((
            StatusCode::CONFLICT,
            Json(ConflictResponse {
                existing_id: existing.id,
                error: "audio already imported".into(),
            }),
        ));
    }

    let result = import_audio(
        Arc::clone(&state.meeting_repo),
        Arc::clone(&state.job_repo),
        Arc::clone(&state.file_store),
        &state.recordings_dir,
        source,
        req.copy,
        req.transcribe,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ConflictResponse {
                existing_id: String::new(),
                error: e.to_string(),
            }),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(ImportResponse {
            id: result.meeting.id,
            name: result.meeting.name,
            audio_path: result.meeting.audio_path.display().to_string(),
            job_id: result.job.map(|j| j.id),
        }),
    ))
}

// ── Scan ("Из папки") ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ScanQuery {
    /// Directory to scan; defaults to the server's recordings_dir.
    pub dir: Option<String>,
}

pub async fn scan(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ScanQuery>,
) -> Result<Json<Vec<ScanCandidate>>, (StatusCode, String)> {
    let dir = q
        .dir
        .map(PathBuf::from)
        .unwrap_or_else(|| state.recordings_dir.clone());

    let candidates = scan_recordings_dir(
        Arc::clone(&state.meeting_repo),
        Arc::clone(&state.file_store),
        &dir,
        DEFAULT_MAX_DEPTH,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(candidates))
}

// ── Reprocess ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ReprocessRequest {
    /// "transcribe" | "protocol".
    pub kind: String,
    pub template_name: Option<String>,
}

#[derive(Serialize)]
pub struct JobIdResponse {
    pub job_id: String,
}

pub async fn reprocess(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ReprocessRequest>,
) -> Result<(StatusCode, Json<JobIdResponse>), (StatusCode, String)> {
    let job = match req.kind.as_str() {
        "transcribe" => {
            reprocess_transcribe(
                Arc::clone(&state.meeting_repo),
                Arc::clone(&state.job_repo),
                &id,
            )
            .await
        }
        "protocol" => {
            // Decision #3: resolve the configured default template in the API
            // layer before the job is enqueued, so the worker's use-case call
            // receives a ready template name and settings never reach the core.
            let template_name = req.template_name.or_else(|| (state.default_template)());
            regenerate_protocol(
                Arc::clone(&state.meeting_repo),
                Arc::clone(&state.job_repo),
                &id,
                template_name,
            )
            .await
        }
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unknown reprocess kind '{other}'"),
            ));
        }
    }
    .map_err(map_core_err)?;

    Ok((StatusCode::ACCEPTED, Json(JobIdResponse { job_id: job.id })))
}

// ── Delete ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeleteQuery {
    /// "audio" | "full" (default "full").
    pub mode: Option<String>,
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<DeleteQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mode = match q.mode.as_deref() {
        Some("audio") => DeleteMode::AudioOnly,
        Some("full") | None => DeleteMode::Full,
        Some(other) => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unknown delete mode '{other}'"),
            ));
        }
    };

    delete_meeting(
        Arc::clone(&state.meeting_repo),
        Arc::clone(&state.file_store),
        &state.recordings_dir,
        &id,
        mode,
    )
    .await
    .map_err(map_core_err)?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_core_err(e: CoreError) -> (StatusCode, String) {
    match e {
        CoreError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        CoreError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use crate::router::{create_router, AppState};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
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
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
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

    #[tokio::test]
    async fn get_returns_persisted_protocol() {
        let repo = FakeMeetingRepo::new();
        let m = Meeting::new("Ретро".to_string(), PathBuf::from("/c.wav"));
        repo.save(&m).await.unwrap();
        repo.save_transcript(&m.id, "текст").await.unwrap();
        repo.save_protocol(&m.id, "# Протокол").await.unwrap();
        let app = make_app(std::sync::Arc::clone(&repo));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/api/v1/meetings/{}", m.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["protocol"], "# Протокол");
        assert_eq!(json["has_transcript"], true);
    }

    #[tokio::test]
    async fn get_without_protocol_returns_null() {
        let repo = FakeMeetingRepo::new();
        let m = Meeting::new("Без протокола".to_string(), PathBuf::from("/d.wav"));
        repo.save(&m).await.unwrap();
        let app = make_app(std::sync::Arc::clone(&repo));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/api/v1/meetings/{}", m.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert!(json["protocol"].is_null());
    }

    #[tokio::test]
    async fn get_unknown_meeting_returns_404() {
        let app = make_app(FakeMeetingRepo::new());

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/meetings/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // ── Phase 3: import / scan / reprocess / delete ─────────────────────────

    fn make_app_full(
        repo: std::sync::Arc<FakeMeetingRepo>,
        job_repo: std::sync::Arc<FakeJobRepo>,
        file_store: std::sync::Arc<FakeMeetingFileStore>,
    ) -> axum::Router {
        create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: repo,
            job_repo,
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store,
            recordings_dir: PathBuf::from("/recordings"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
        })
    }

    async fn post_json(
        app: axum::Router,
        uri: &str,
        body: serde_json::Value,
    ) -> axum::response::Response {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn import_creates_meeting_and_job() {
        let repo = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let fs = FakeMeetingFileStore::new();
        let app = make_app_full(
            std::sync::Arc::clone(&repo),
            std::sync::Arc::clone(&jr),
            std::sync::Arc::clone(&fs),
        );

        let resp = post_json(
            app,
            "/api/v1/meetings/import",
            serde_json::json!({ "path": "/downloads/talk.wav" }),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["name"], "talk");
        assert!(json["job_id"].as_str().is_some());
        assert_eq!(repo.list_all().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn import_duplicate_audio_path_returns_409_with_existing_id() {
        let repo = FakeMeetingRepo::new();
        // Register a meeting in place so its audio_path equals the import path.
        let existing = Meeting::new("Existing".into(), PathBuf::from("/recordings/dup.wav"));
        repo.save(&existing).await.unwrap();

        let jr = FakeJobRepo::new();
        let fs = FakeMeetingFileStore::new();
        let app = make_app_full(
            std::sync::Arc::clone(&repo),
            std::sync::Arc::clone(&jr),
            std::sync::Arc::clone(&fs),
        );

        let resp = post_json(
            app,
            "/api/v1/meetings/import",
            serde_json::json!({ "path": "/recordings/dup.wav", "copy": false }),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::CONFLICT);
        let json = body_json(resp).await;
        assert_eq!(json["existing_id"], existing.id);
        // No second meeting created.
        assert_eq!(repo.list_all().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn scan_returns_untracked_files_only() {
        let repo = FakeMeetingRepo::new();
        let tracked = Meeting::new("Tracked".into(), PathBuf::from("/recordings/a.wav"));
        repo.save(&tracked).await.unwrap();

        let jr = FakeJobRepo::new();
        let fs = FakeMeetingFileStore::with_audio_files([
            PathBuf::from("/recordings/a.wav"),
            PathBuf::from("/recordings/b.m4a"),
        ]);
        let app = make_app_full(
            std::sync::Arc::clone(&repo),
            std::sync::Arc::clone(&jr),
            std::sync::Arc::clone(&fs),
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/meetings/scan?dir=/recordings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "b.m4a");
    }

    #[tokio::test]
    async fn delete_audio_only_keeps_meeting_listed() {
        let repo = FakeMeetingRepo::new();
        let m = Meeting::new("M".into(), PathBuf::from("/recordings/x/audio.wav"));
        repo.save(&m).await.unwrap();
        repo.save_transcript(&m.id, "kept").await.unwrap();

        let jr = FakeJobRepo::new();
        let fs = FakeMeetingFileStore::new();
        let app = make_app_full(
            std::sync::Arc::clone(&repo),
            std::sync::Arc::clone(&jr),
            std::sync::Arc::clone(&fs),
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/meetings/{}?mode=audio", m.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let after = repo.find_by_id(&m.id).await.unwrap().unwrap();
        assert!(after.audio_path.as_os_str().is_empty());
        assert_eq!(after.transcript_text.as_deref(), Some("kept"));
        assert!(fs
            .removed_files
            .lock()
            .unwrap()
            .contains(&PathBuf::from("/recordings/x/audio.wav")));
    }

    #[tokio::test]
    async fn reprocess_unknown_meeting_returns_404() {
        let repo = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let fs = FakeMeetingFileStore::new();
        let app = make_app_full(repo, jr, fs);

        let resp = post_json(
            app,
            "/api/v1/meetings/ghost/reprocess",
            serde_json::json!({ "kind": "transcribe" }),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
