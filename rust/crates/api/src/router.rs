use axum::routing::put;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use meeting_core::ports::{
    AudioCapture, JobRepo, LlmProvider, MeetingFileStore, MeetingRepo, TemplateLoader, Transcriber,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::GlobalKeyExtractor, GovernorLayer,
};

pub use meeting_core::LiveJobs;

/// Resolves the configured `default_template` name when a protocol request
/// omits one. Decision #3: settings never leak into the core; the API layer
/// resolves the name (reading the live settings store) **before** calling the
/// `generate_protocol` use-case, which only ever sees a ready `template_name`.
pub type DefaultTemplateFn = Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// A resolver that always yields no default template. Used by per-route unit
/// tests and the legacy CLI, which pass template names explicitly.
pub fn no_default_template() -> DefaultTemplateFn {
    Arc::new(|| None)
}
pub use crate::routes::audio::AudioApi;
use crate::routes::{
    audio, health, jobs, meetings, protocols, recordings, settings, templates,
    transcription_models, version,
};
use crate::settings_service::SettingsService;
use crate::template_service::TemplateService;
use crate::transcription_model_service::TranscriptionModelService;

pub struct AppState {
    pub transcriber: Arc<dyn Transcriber>,
    pub meeting_repo: Arc<dyn MeetingRepo>,
    pub job_repo: Arc<dyn JobRepo>,
    pub llm: Arc<dyn LlmProvider>,
    pub templates: Arc<dyn TemplateLoader>,
    pub audio_capture: Arc<dyn AudioCapture>,
    pub file_store: Arc<dyn MeetingFileStore>,
    /// Directory where per-meeting recording subdirs are created.
    pub recordings_dir: PathBuf,
    /// Live, in-memory job table (shared with the worker). Each entry holds
    /// both the in-flight progress snapshot and a cancellation token; the
    /// `DELETE /api/v1/jobs/:id` handler signals the token to stop a running
    /// job at its next safe checkpoint.
    pub progress: LiveJobs,
    /// Resolves the configured default template when a protocol request omits
    /// one (decision #3 — the API layer resolves it before the use-case runs).
    pub default_template: DefaultTemplateFn,
}

/// Payload of `GET /version`. Build version is informational; the protocol
/// range is the compatibility key (see [`crate::PROTOCOL_VERSION`]).
#[derive(Clone, Serialize)]
pub struct VersionInfo {
    pub build: String,
    pub protocol: u32,
    pub min_protocol: u32,
}

/// Builds the API routes only, with **no auth** middleware.
///
/// Retained for per-route unit tests that exercise handler logic directly.
/// Production traffic must go through [`create_server_router`].
pub fn create_router(state: AppState) -> Router {
    api_routes().with_state(Arc::new(state))
}

/// Builds the full sidecar router:
/// - the 7 `/api/v1/*` routes, each gated by a `Bearer <auth_token>` check
///   (401 without a valid token);
/// - `GET /health` and `GET /version`, both **unauthenticated**.
///
/// `auth_token` is generated fresh per process and delivered only via the
/// stdout handshake — never argv, never logged.
pub fn create_server_router(
    state: AppState,
    settings_service: Arc<dyn SettingsService>,
    template_service: Arc<dyn TemplateService>,
    transcription_model_service: Arc<dyn TranscriptionModelService>,
    audio_api: Arc<AudioApi>,
    auth_token: String,
    build_version: impl Into<String>,
) -> Router {
    let token: Arc<str> = Arc::from(auth_token.as_str());

    let api = api_routes()
        .route_layer(middleware::from_fn_with_state(
            token.clone(),
            require_bearer,
        ))
        .with_state(Arc::new(state));

    // Settings routes carry their own state (the SettingsService), kept off
    // AppState so existing route tests stay untouched. Same bearer gate.
    let settings = settings_routes()
        .route_layer(middleware::from_fn_with_state(
            token.clone(),
            require_bearer,
        ))
        .with_state(settings_service);

    // Template routes follow the same separate-state pattern (the
    // TemplateService), for the same reason. Same bearer gate.
    let templates = template_routes()
        .route_layer(middleware::from_fn_with_state(
            token.clone(),
            require_bearer,
        ))
        .with_state(template_service);

    let transcription_models = transcription_model_routes()
        .route_layer(middleware::from_fn_with_state(
            token.clone(),
            require_bearer,
        ))
        .with_state(transcription_model_service);

    // Audio routes (device enumeration + live level monitor) carry their own
    // state, same separate-state + bearer pattern as settings/templates.
    let audio = audio_routes()
        .route_layer(middleware::from_fn_with_state(token, require_bearer))
        .with_state(audio_api);

    let meta = Router::new()
        .route("/health", get(health::handle))
        .route("/version", get(version::handle))
        .with_state(VersionInfo {
            build: build_version.into(),
            protocol: crate::PROTOCOL_VERSION,
            min_protocol: crate::MIN_PROTOCOL_VERSION,
        });

    // Global rate limit on the authenticated surface. On loopback every peer is
    // 127.0.0.1, so the legitimate GUI and a co-tenant abuser are
    // indistinguishable by IP — a single global GCRA bucket (not per-IP, which
    // would be identical here but demand ConnectInfo plumbing) is the keying
    // that matches the threat. Steady ~100 req/s (one cell per 10 ms) with a
    // burst of 50; the GUI sits far under this by design. The default handler
    // rejects excess with `429 Too Many Requests` and an `x-ratelimit-after`
    // retry hint. See plans/api-rate-limit/plan.md for the full rationale.
    let governor = GovernorLayer {
        config: Arc::new(
            GovernorConfigBuilder::default()
                .per_millisecond(10)
                .burst_size(50)
                .key_extractor(GlobalKeyExtractor)
                .finish()
                .expect("rate-limit period and burst are non-zero"),
        ),
    };

    // The limiter is applied *outside* the per-router `require_bearer` gate
    // (`.layer` wraps the already-merged auth routers, so it runs first). A
    // brute-force loop that only ever yields 401s therefore still drains the
    // bucket and trips the limiter. `/health` and `/version` are merged after
    // and stay unthrottled.
    let authed = api
        .merge(settings)
        .merge(templates)
        .merge(transcription_models)
        .merge(audio)
        .layer(governor);

    authed.merge(meta)
}

fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/jobs", post(jobs::submit))
        .route("/api/v1/active-jobs", get(jobs::active))
        .route("/api/v1/jobs/:id/events", get(jobs::events))
        .route("/api/v1/jobs/:id", get(jobs::status).delete(jobs::cancel))
        .route("/api/v1/protocols", post(protocols::generate))
        .route("/api/v1/recordings", post(recordings::start))
        .route("/api/v1/recordings/:id/stop", post(recordings::stop))
        .route("/api/v1/meetings", get(meetings::list))
        .route("/api/v1/meetings/import", post(meetings::import))
        .route("/api/v1/meetings/scan", get(meetings::scan))
        .route("/api/v1/meetings/:id/reprocess", post(meetings::reprocess))
        .route(
            "/api/v1/meetings/:id",
            get(meetings::get).delete(meetings::delete),
        )
}

fn settings_routes() -> Router<Arc<dyn SettingsService>> {
    Router::new()
        .route("/api/v1/settings", get(settings::get).put(settings::put))
        .route("/api/v1/settings/secret", put(settings::put_secret))
        .route("/api/v1/settings/test", post(settings::test))
        .route(
            "/api/v1/settings/secret-store/protect",
            post(settings::protect_secret_store),
        )
        .route(
            "/api/v1/settings/secret-store/unlock",
            post(settings::unlock_secret_store),
        )
        .route(
            "/api/v1/settings/secret-store/lock",
            post(settings::lock_secret_store),
        )
        .route(
            "/api/v1/settings/secret-store/passphrase",
            post(settings::change_secret_passphrase),
        )
        .route(
            "/api/v1/settings/secret-store/migrate",
            post(settings::migrate_secret_store),
        )
        .route(
            "/api/v1/settings/secret-store/reset",
            post(settings::reset_secret_store),
        )
}

fn audio_routes() -> Router<Arc<AudioApi>> {
    Router::new()
        .route("/api/v1/audio/devices", get(audio::devices))
        .route("/api/v1/audio/monitor", post(audio::monitor_start))
        .route(
            "/api/v1/audio/monitor/:id",
            get(audio::monitor_level).delete(audio::monitor_stop),
        )
}

fn template_routes() -> Router<Arc<dyn TemplateService>> {
    Router::new()
        .route("/api/v1/templates", get(templates::list))
        .route(
            "/api/v1/templates/:name",
            get(templates::get)
                .put(templates::put)
                .delete(templates::delete),
        )
        .route("/api/v1/templates/:name/rename", post(templates::rename))
}

fn transcription_model_routes() -> Router<Arc<dyn TranscriptionModelService>> {
    Router::new()
        .route(
            "/api/v1/transcription-models",
            get(transcription_models::list),
        )
        .route(
            "/api/v1/transcription-models/:id/install",
            post(transcription_models::install),
        )
        .route(
            "/api/v1/transcription-models/installations/:job_id",
            get(transcription_models::installation),
        )
        .route(
            "/api/v1/transcription-models/:id",
            axum::routing::delete(transcription_models::delete),
        )
}

/// Rejects any request to an `/api/*` route that lacks a matching
/// `Authorization: Bearer <token>` header with `401 Unauthorized`.
async fn require_bearer(
    State(expected): State<Arc<str>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let provided = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match provided {
        Some(tok) if constant_time_eq(tok.as_bytes(), expected.as_bytes()) => {
            Ok(next.run(req).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Constant-time equality over the token bytes. The length is allowed to leak
/// (the token is a fixed-width 64-hex-char value — its length is public); only
/// the secret content must not be distinguishable via response timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use meeting_core::fakes::{
        FakeAudioCapture, FakeJobRepo, FakeLlmProvider, FakeMeetingFileStore, FakeMeetingRepo,
        FakeTemplateLoader, FakeTranscriber,
    };
    use std::path::PathBuf;
    use tower::ServiceExt;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    struct FakeSettings;
    #[async_trait::async_trait]
    impl SettingsService for FakeSettings {
        fn snapshot(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn update(&self, _body: serde_json::Value) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }
        async fn set_secret(
            &self,
            _provider: String,
            _value: Option<String>,
        ) -> Result<(), String> {
            Ok(())
        }
        async fn test_provider(&self, _provider: String) -> Result<(), String> {
            Ok(())
        }
        async fn protect_secret_store(&self, _passphrase: String) -> Result<(), String> {
            Ok(())
        }
        async fn unlock_secret_store(&self, _passphrase: String) -> Result<(), String> {
            Ok(())
        }
        async fn lock_secret_store(&self) -> Result<(), String> {
            Ok(())
        }
        async fn change_secret_passphrase(&self, _old: String, _new: String) -> Result<(), String> {
            Ok(())
        }
        async fn migrate_secret_store_to_keyring(&self, _passphrase: String) -> Result<(), String> {
            Ok(())
        }
        async fn reset_secret_store(&self) -> Result<(), String> {
            Ok(())
        }
    }

    struct FakeTemplates;
    #[async_trait::async_trait]
    impl TemplateService for FakeTemplates {
        async fn list(&self) -> Result<Vec<crate::TemplateDto>, crate::TemplateError> {
            Ok(Vec::new())
        }
        async fn get(&self, _name: &str) -> Result<String, crate::TemplateError> {
            Ok(String::new())
        }
        async fn save(&self, _name: &str, _body: &str) -> Result<(), crate::TemplateError> {
            Ok(())
        }
        async fn delete(&self, _name: &str) -> Result<Option<String>, crate::TemplateError> {
            Ok(None)
        }
        async fn rename(&self, _old: &str, _new: &str) -> Result<(), crate::TemplateError> {
            Ok(())
        }
    }

    struct FakeTranscriptionModels;
    #[async_trait::async_trait]
    impl TranscriptionModelService for FakeTranscriptionModels {
        async fn list(&self) -> Result<crate::TranscriptionModelsView, crate::ModelServiceError> {
            Ok(crate::TranscriptionModelsView {
                models: Vec::new(),
                selected_model_id: None,
                active_source: "managed".to_string(),
                models_dir: "/tmp/models".to_string(),
            })
        }

        async fn start_install(
            &self,
            _model_id: String,
        ) -> Result<crate::InstallStarted, crate::ModelServiceError> {
            Ok(crate::InstallStarted {
                job_id: "job-1".to_string(),
            })
        }

        async fn installation(
            &self,
            job_id: String,
        ) -> Result<crate::InstallationView, crate::ModelServiceError> {
            Ok(crate::InstallationView {
                job_id,
                model_id: "base".to_string(),
                status: "done".to_string(),
                bytes_downloaded: 1,
                total_bytes: Some(1),
                percent: Some(100.0),
                error: None,
            })
        }

        async fn delete_model(&self, _model_id: String) -> Result<(), crate::ModelServiceError> {
            Ok(())
        }
    }

    fn server() -> Router {
        create_server_router(
            AppState {
                transcriber: FakeTranscriber::new("fake"),
                meeting_repo: FakeMeetingRepo::new(),
                job_repo: FakeJobRepo::new(),
                llm: FakeLlmProvider::new(""),
                templates: FakeTemplateLoader::empty(),
                audio_capture: FakeAudioCapture::new(),
                file_store: FakeMeetingFileStore::new(),
                recordings_dir: PathBuf::from("/tmp"),
                progress: meeting_core::LiveProgress::new(),
                default_template: super::no_default_template(),
            },
            Arc::new(FakeSettings),
            Arc::new(FakeTemplates),
            Arc::new(FakeTranscriptionModels),
            Arc::new(AudioApi {
                enumerator: meeting_core::fakes::FakeAudioDeviceEnumerator::new(),
                monitor: meeting_core::fakes::FakeAudioLevelMonitor::new(),
            }),
            TOKEN.to_string(),
            "0.1.0-test",
        )
    }

    // The API routes the contract requires to be auth-gated.
    const API_ROUTES: &[(&str, &str)] = &[
        ("POST", "/api/v1/jobs"),
        ("GET", "/api/v1/active-jobs"),
        ("GET", "/api/v1/jobs/abc/events"),
        ("GET", "/api/v1/jobs/abc"),
        ("DELETE", "/api/v1/jobs/abc"),
        ("POST", "/api/v1/protocols"),
        ("POST", "/api/v1/recordings"),
        ("POST", "/api/v1/recordings/abc/stop"),
        ("GET", "/api/v1/meetings"),
        ("POST", "/api/v1/meetings/import"),
        ("GET", "/api/v1/meetings/scan"),
        ("POST", "/api/v1/meetings/abc/reprocess"),
        ("DELETE", "/api/v1/meetings/abc"),
        ("GET", "/api/v1/settings"),
        ("PUT", "/api/v1/settings"),
        ("PUT", "/api/v1/settings/secret"),
        ("POST", "/api/v1/settings/test"),
        ("GET", "/api/v1/templates"),
        ("GET", "/api/v1/templates/foo"),
        ("PUT", "/api/v1/templates/foo"),
        ("DELETE", "/api/v1/templates/foo"),
        ("POST", "/api/v1/templates/foo/rename"),
        ("GET", "/api/v1/transcription-models"),
        ("POST", "/api/v1/transcription-models/base/install"),
        ("GET", "/api/v1/transcription-models/installations/job-1"),
        ("DELETE", "/api/v1/transcription-models/base"),
        ("GET", "/api/v1/audio/devices"),
        ("POST", "/api/v1/audio/monitor"),
        ("GET", "/api/v1/audio/monitor/abc"),
        ("DELETE", "/api/v1/audio/monitor/abc"),
    ];

    #[tokio::test]
    async fn all_seven_api_routes_401_without_token() {
        for (method, path) in API_ROUTES {
            let resp = server()
                .oneshot(
                    Request::builder()
                        .method(*method)
                        .uri(*path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {path} must be 401 without a bearer token"
            );
        }
    }

    #[tokio::test]
    async fn api_route_passes_auth_with_valid_token() {
        // GET /api/v1/meetings on fakes returns 200 once auth passes.
        let resp = server()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/meetings")
                    .header("Authorization", format!("Bearer {TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_route_401_with_wrong_token() {
        let resp = server()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/meetings")
                    .header("Authorization", "Bearer not-the-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_needs_no_auth() {
        let resp = server()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "ok");
    }

    #[tokio::test]
    async fn brute_force_401s_are_rate_limited() {
        // A co-tenant guessing tokens never gets past `require_bearer`, but the
        // limiter sits ahead of it: enough 401-yielding requests must exhaust
        // the global bucket and start returning 429. Burst is 50, so well over
        // that count guarantees the limiter trips. All calls share one router
        // instance so they hit the same Arc'd GCRA bucket.
        let app = server();
        let mut saw_429 = false;
        for _ in 0..200 {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/api/v1/meetings")
                        .header("Authorization", "Bearer not-the-token")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            match resp.status() {
                StatusCode::UNAUTHORIZED => {}
                StatusCode::TOO_MANY_REQUESTS => {
                    saw_429 = true;
                    break;
                }
                other => panic!("unexpected status {other} during brute force"),
            }
        }
        assert!(
            saw_429,
            "brute-force 401 traffic must trip the rate limiter (429)"
        );
    }

    #[tokio::test]
    async fn health_is_not_rate_limited() {
        // `/health` is merged outside the limiter; hammering it past the burst
        // must never yield 429.
        let app = server();
        for _ in 0..200 {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/health")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::OK,
                "/health must never be rate-limited"
            );
        }
    }

    #[tokio::test]
    async fn version_needs_no_auth_and_carries_protocol_range() {
        let resp = server()
            .oneshot(
                Request::builder()
                    .uri("/version")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["build"], "0.1.0-test");
        assert_eq!(v["protocol"], crate::PROTOCOL_VERSION);
        assert_eq!(v["min_protocol"], crate::MIN_PROTOCOL_VERSION);
    }
}
