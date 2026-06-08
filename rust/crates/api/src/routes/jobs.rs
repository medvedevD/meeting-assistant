use crate::router::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_core::Stream;
use meeting_core::{
    entities::{Job, JobProgress},
    ports::JobRepo,
    usecases::{
        cancel_job as cancel_job_usecase, get_job_status, list_active_jobs,
        submit_transcription_job, CancelOutcome,
    },
    ProgressEvent,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

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

/// `GET /api/v1/jobs/:id/events` — Server-Sent Events stream of a job's live
/// progress (see `plans/done/job-progress-sse` ADR-002). Pushes the current
/// snapshot on connect, a `progress` frame per worker update, then a single
/// terminal `status` frame carrying the persisted final state before closing.
/// The polling path (`GET /jobs/:id`) is retained as a fallback for clients or
/// environments where SSE is unavailable.
///
/// Wire protocol — every `data:` is a `JobResponse` JSON, identical to the
/// polling shape, so the client reuses a single decode path:
/// - `event: status`   — the initial snapshot and the terminal frame.
/// - `event: progress` — a live in-flight update (status `running`).
pub async fn events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    // Subscribe *before* reading the initial snapshot, so an update landing in
    // the gap is queued rather than lost (a duplicate first frame is harmless).
    let mut rx = state.progress.subscribe();

    let job = get_job_status(Arc::clone(&state.job_repo), &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("job {id} not found")))?;

    // Immutable for the life of the stream — cached so `progress` frames never
    // hit the DB. The status/error_class are re-read from the DB only once, at
    // terminal, where they actually change.
    let meeting_id = job.meeting_id.clone();
    let kind = job.kind.as_str().to_string();

    let mut initial: JobResponse = job.into();
    initial.progress = state.progress.get(&id).map(|e| e.value().progress.clone());
    let already_terminal = matches!(initial.status.as_str(), "done" | "failed");

    let job_repo = Arc::clone(&state.job_repo);
    let live = Arc::clone(&state.progress);

    let stream = async_stream::stream! {
        yield Ok(sse_event("status", &initial));
        // A job that is already done/failed at connect needs no live feed.
        if already_terminal {
            return;
        }
        loop {
            match rx.recv().await {
                Ok(ProgressEvent::Snapshot { job_id, progress }) if job_id == id => {
                    yield Ok(sse_event("progress", &running_frame(&id, &meeting_id, &kind, progress)));
                }
                Ok(ProgressEvent::Terminal { job_id }) if job_id == id => {
                    if let Some(frame) = terminal_frame(&job_repo, &id).await {
                        yield Ok(sse_event("status", &frame));
                    }
                    break;
                }
                // An event for a different job — ignore and keep listening.
                Ok(_) => {}
                // Fell behind the channel: resync from current state instead of
                // trusting the (dropped) payloads. A vanished entry means the
                // job went terminal while we lagged.
                Err(RecvError::Lagged(_)) => {
                    match live.get(&id).map(|e| e.value().progress.clone()) {
                        Some(progress) => {
                            yield Ok(sse_event("progress", &running_frame(&id, &meeting_id, &kind, progress)));
                        }
                        None => {
                            if let Some(frame) = terminal_frame(&job_repo, &id).await {
                                yield Ok(sse_event("status", &frame));
                            }
                            break;
                        }
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Serialize a `JobResponse` as a named SSE event. Serialization of this plain
/// data struct cannot fail in practice; on the impossible error we emit a
/// comment frame so the stream stays well-formed.
fn sse_event(name: &str, resp: &JobResponse) -> Event {
    Event::default()
        .event(name)
        .json_data(resp)
        .unwrap_or_else(|_| Event::default().comment("serialization error"))
}

/// A live in-flight frame: `running` status with the current progress and the
/// immutable identity fields. attempts/timestamps are not surfaced live — while
/// running the client reads only `status` and `progress`.
fn running_frame(id: &str, meeting_id: &str, kind: &str, progress: JobProgress) -> JobResponse {
    JobResponse {
        id: id.to_string(),
        meeting_id: meeting_id.to_string(),
        kind: kind.to_string(),
        status: "running".to_string(),
        attempts: 0,
        last_error: None,
        error_class: None,
        progress: Some(progress),
        created_at: 0,
        updated_at: 0,
    }
}

/// Read the persisted terminal job for the final `status` frame — the single DB
/// read at end-of-stream. `None` (row vanished) simply closes the stream.
async fn terminal_frame(job_repo: &Arc<dyn JobRepo>, id: &str) -> Option<JobResponse> {
    match get_job_status(Arc::clone(job_repo), id).await {
        Ok(Some(job)) => Some(job.into()),
        _ => None,
    }
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
            progress: meeting_core::LiveProgress::new(),
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
            progress: meeting_core::LiveProgress::new(),
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
        let progress: crate::router::LiveJobs = meeting_core::LiveProgress::new();

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
        let progress: crate::router::LiveJobs = meeting_core::LiveProgress::new();

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
            progress: meeting_core::LiveProgress::new(),
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
            progress: meeting_core::LiveProgress::new(),
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
            meeting_core::LiveProgress::new();
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

    // ── GET /api/v1/jobs/:id/events (SSE) ────────────────────────────────────

    fn app_with(jr: Arc<FakeJobRepo>, progress: crate::router::LiveJobs) -> axum::Router {
        create_router(AppState {
            transcriber: FakeTranscriber::new("fake"),
            meeting_repo: FakeMeetingRepo::new(),
            job_repo: jr as Arc<dyn meeting_core::ports::JobRepo>,
            llm: FakeLlmProvider::new(""),
            templates: FakeTemplateLoader::empty(),
            audio_capture: FakeAudioCapture::new(),
            file_store: FakeMeetingFileStore::new(),
            recordings_dir: std::path::PathBuf::from("/tmp"),
            progress,
            default_template: crate::router::no_default_template(),
        })
    }

    async fn body_text(response: axum::response::Response) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn events_unknown_job_returns_404() {
        let app = make_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/jobs/ghost/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn events_finished_job_emits_terminal_status_then_closes() {
        use meeting_core::entities::Job;
        use meeting_core::ports::JobRepo;

        let jr = FakeJobRepo::new();
        let done = Job::new_transcribe("m1".into());
        jr.enqueue(&done).await.unwrap();
        jr.mark_done(&done.id, 1).await.unwrap();

        let app = app_with(Arc::clone(&jr), meeting_core::LiveProgress::new());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/jobs/{}/events", done.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(ct.starts_with("text/event-stream"), "content-type: {ct}");

        // An already-terminal job yields exactly one `status` frame, then the
        // stream completes (so `collect()` returns instead of hanging).
        let body = body_text(resp).await;
        assert!(body.contains("event: status"), "body: {body}");
        assert!(body.contains("\"status\":\"done\""), "body: {body}");
        assert!(body.contains(&format!("\"id\":\"{}\"", done.id)), "body: {body}");
    }

    #[tokio::test]
    async fn events_streams_live_progress_then_terminal() {
        use meeting_core::entities::{Job, JobProgress, PipelineStage};
        use meeting_core::ports::JobRepo;
        use meeting_core::LiveEntry;
        use tokio_util::sync::CancellationToken;

        let jr = FakeJobRepo::new();
        let job = Job::new_transcribe("m1".into());
        jr.enqueue(&job).await.unwrap();
        jr.claim_pending(i64::MAX).await.unwrap(); // → running

        let progress = meeting_core::LiveProgress::new();
        progress.seed(
            job.id.clone(),
            LiveEntry {
                progress: JobProgress::new(PipelineStage::Queued, "В очереди", 0),
                cancel: CancellationToken::new(),
            },
        );

        let app = app_with(Arc::clone(&jr), Arc::clone(&progress));
        let uri = format!("/api/v1/jobs/{}/events", job.id);

        // Drive the stream concurrently: a writer pushes a mid-flight update and
        // then drives the job to terminal, which ends the stream.
        let collect = tokio::spawn(async move {
            let resp = app
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            body_text(resp).await
        });

        // Let the handler subscribe + emit the initial frame before we publish.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        progress.publish(
            &job.id,
            JobProgress::new(PipelineStage::Transcribing, "Распознавание речи", 55),
        );
        jr.mark_done(&job.id, 1).await.unwrap();
        progress.finish(&job.id); // terminal → stream emits final status + closes

        let body = tokio::time::timeout(std::time::Duration::from_secs(2), collect)
            .await
            .expect("stream must close after terminal")
            .unwrap();

        assert!(body.contains("event: progress"), "body: {body}");
        assert!(body.contains("\"percent\":55"), "body: {body}");
        // Final frame carries the persisted terminal status.
        assert!(body.contains("\"status\":\"done\""), "body: {body}");
    }
}
