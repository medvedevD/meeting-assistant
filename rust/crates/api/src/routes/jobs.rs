use crate::router::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use meeting_core::{
    entities::{Job, JobProgress},
    usecases::{
        cancel_job as cancel_job_usecase, get_job_status, list_active_jobs,
        submit_transcription_job, CancelOutcome,
    },
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

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
    /// Classified terminal failure (persisted), for the UI error banner.
    pub error_class: Option<String>,
    /// Live pipeline progress, present only while the job is active (in-memory).
    pub progress: Option<JobProgress>,
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
            error_class: j.error_class.map(|c| c.as_str().to_string()),
            progress: None,
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

    // Merge the persisted job (status + error_class) with the live in-memory
    // progress, present only while the job is mid-flight (decision #11).
    let mut resp: JobResponse = job.into();
    resp.progress = state.progress.get(&id).map(|e| e.value().progress.clone());

    Ok(Json(resp))
}

/// Cooperative cancellation of a single job. Pending jobs flip to terminal
/// `failed` (`error_class='cancelled'`) in a single repo write and respond
/// `202`; running jobs have their in-memory cancellation token signalled and
/// the worker drives the terminal transition at its next safe checkpoint
/// (also `202`). Already-terminal jobs return `204`; unknown ids return `404`.
/// See `plans/active/job-cancellation`.
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let outcome = cancel_job_usecase(Arc::clone(&state.job_repo), Arc::clone(&state.progress), &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match outcome {
        CancelOutcome::NotFound => Err((StatusCode::NOT_FOUND, format!("job {id} not found"))),
        CancelOutcome::Cancelled | CancelOutcome::Cancelling => Ok(StatusCode::ACCEPTED),
        CancelOutcome::AlreadyTerminal => Ok(StatusCode::NO_CONTENT),
    }
}

/// In-flight jobs (`pending` or `running`), oldest first. Lets the UI re-seed
/// its active-jobs view after an app restart; each entry merges any live
/// in-memory progress just like `status`.
pub async fn active(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<JobResponse>>, (StatusCode, String)> {
    let jobs = list_active_jobs(Arc::clone(&state.job_repo))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let resp = jobs
        .into_iter()
        .map(|job| {
            let mut r: JobResponse = job.into();
            r.progress = state.progress.get(&r.id).map(|e| e.value().progress.clone());
            r
        })
        .collect();

    Ok(Json(resp))
}

#[cfg(test)]
mod tests {
    use super::*;
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
            llm: FakeLlmProvider::new(""),
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
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
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
        // No live progress entry → null; no terminal failure → null error_class.
        assert!(status_json["progress"].is_null());
        assert!(status_json["error_class"].is_null());
    }

    #[tokio::test]
    async fn active_lists_only_in_flight_jobs_with_merged_progress() {
        use meeting_core::entities::{Job, JobProgress, PipelineStage};
        use meeting_core::ports::JobRepo;
        use meeting_core::LiveEntry;
        use tokio_util::sync::CancellationToken;

        let jr = FakeJobRepo::new();
        let progress: crate::router::LiveJobs = std::sync::Arc::new(dashmap::DashMap::new());

        // One pending job with live progress, one done (excluded).
        let mut pending = Job::new_transcribe("m1".into());
        pending.created_at = 100;
        jr.enqueue(&pending).await.unwrap();
        progress.insert(
            pending.id.clone(),
            LiveEntry {
                progress: JobProgress::new(PipelineStage::Transcribing, "Распознавание речи", 42),
                cancel: CancellationToken::new(),
            },
        );

        let done = Job::new_transcribe("m2".into());
        jr.enqueue(&done).await.unwrap();
        jr.mark_done(&done.id, 1).await.unwrap();

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: Arc::clone(&jr) as Arc<dyn JobRepo>,
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: Arc::clone(&progress),
            default_template: crate::router::no_default_template(),
        });

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/active-jobs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        let arr = json.as_array().expect("array");
        assert_eq!(arr.len(), 1, "only the in-flight job is returned");
        assert_eq!(arr[0]["id"], pending.id);
        assert_eq!(arr[0]["meeting_id"], "m1");
        assert_eq!(arr[0]["progress"]["percent"], 42);
    }

    #[tokio::test]
    async fn status_merges_live_progress_and_persisted_error_class() {
        use meeting_core::entities::{Job, JobProgress, PipelineStage};
        use meeting_core::ports::JobRepo;
        use meeting_core::LiveEntry;
        use tokio_util::sync::CancellationToken;

        let jr = FakeJobRepo::new();
        let progress: crate::router::LiveJobs = std::sync::Arc::new(dashmap::DashMap::new());

        // A claimed job with a live progress entry, plus a (separate) failed job
        // carrying a persisted error_class.
        let active = Job::new_transcribe("m1".into());
        jr.enqueue(&active).await.unwrap();
        progress.insert(
            active.id.clone(),
            LiveEntry {
                progress: JobProgress::new(PipelineStage::Transcribing, "Распознавание речи", 42),
                cancel: CancellationToken::new(),
            },
        );

        let failed = Job::new_transcribe("m2".into());
        jr.enqueue(&failed).await.unwrap();
        jr.mark_permanently_failed(&failed.id, "boom", Some("api_auth"), 5, 1)
            .await
            .unwrap();

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: Arc::clone(&jr) as Arc<dyn JobRepo>,
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: Arc::clone(&progress),
            default_template: crate::router::no_default_template(),
        });

        // Active job: live progress merged in.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/jobs/{}", active.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["progress"]["stage"], "transcribing");
        assert_eq!(json["progress"]["percent"], 42);
        assert!(json["error_class"].is_null());

        // Failed job: error_class from DB, no live progress.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/jobs/{}", failed.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["status"], "failed");
        assert_eq!(json["error_class"], "api_auth");
        assert!(json["progress"].is_null());
    }

    // ── DELETE /api/v1/jobs/:id ──────────────────────────────────────────────

    fn delete_request(id: &str) -> Request<Body> {
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/jobs/{id}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn delete_unknown_returns_404() {
        let app = make_app();
        let resp = app.oneshot(delete_request("ghost")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_pending_returns_202_and_persists_cancelled() {
        use meeting_core::entities::{ErrorClass, Job, JobStatus};
        use meeting_core::ports::JobRepo;

        let jr = FakeJobRepo::new();
        let pending = Job::new_transcribe("m1".into());
        jr.enqueue(&pending).await.unwrap();

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: Arc::clone(&jr) as Arc<dyn JobRepo>,
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
        });

        let resp = app.oneshot(delete_request(&pending.id)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        let in_db = jr.find_by_id(&pending.id).await.unwrap().unwrap();
        assert_eq!(in_db.status, JobStatus::Failed);
        assert_eq!(in_db.error_class, Some(ErrorClass::Cancelled));
    }

    #[tokio::test]
    async fn delete_done_returns_204() {
        use meeting_core::entities::Job;
        use meeting_core::ports::JobRepo;

        let jr = FakeJobRepo::new();
        let done = Job::new_transcribe("m1".into());
        jr.enqueue(&done).await.unwrap();
        jr.mark_done(&done.id, 1).await.unwrap();

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: Arc::clone(&jr) as Arc<dyn JobRepo>,
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: std::sync::Arc::new(dashmap::DashMap::new()),
            default_template: crate::router::no_default_template(),
        });

        let resp = app.oneshot(delete_request(&done.id)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delete_running_returns_202_and_signals_token() {
        use meeting_core::entities::{Job, JobProgress, PipelineStage};
        use meeting_core::ports::JobRepo;
        use meeting_core::LiveEntry;
        use tokio_util::sync::CancellationToken;

        let jr = FakeJobRepo::new();
        let running = Job::new_transcribe("m1".into());
        jr.enqueue(&running).await.unwrap();
        jr.claim_pending(i64::MAX).await.unwrap();

        let progress: crate::router::LiveJobs =
            std::sync::Arc::new(dashmap::DashMap::new());
        let token = CancellationToken::new();
        progress.insert(
            running.id.clone(),
            LiveEntry {
                progress: JobProgress::new(PipelineStage::Transcribing, "Распознавание речи", 50),
                cancel: token.clone(),
            },
        );

        let app = create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: Arc::clone(&jr) as Arc<dyn JobRepo>,
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress: Arc::clone(&progress),
            default_template: crate::router::no_default_template(),
        });

        let resp = app.oneshot(delete_request(&running.id)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        assert!(token.is_cancelled(), "running job's token must be signalled");
    }
}
