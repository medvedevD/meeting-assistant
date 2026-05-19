use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use meeting_core::ports::{AudioCapture, JobRepo, LlmProvider, MeetingRepo, TemplateLoader, Transcriber};
use crate::routes::{transcribe, jobs, protocols, recordings, meetings, health, version};

pub struct AppState {
    pub transcriber: Arc<dyn Transcriber>,
    pub meeting_repo: Arc<dyn MeetingRepo>,
    pub job_repo: Arc<dyn JobRepo>,
    pub llm: Arc<dyn LlmProvider>,
    pub templates: Arc<dyn TemplateLoader>,
    pub audio_capture: Arc<dyn AudioCapture>,
    /// Directory where per-meeting recording subdirs are created.
    pub recordings_dir: PathBuf,
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
    auth_token: String,
    build_version: impl Into<String>,
) -> Router {
    let token: Arc<str> = Arc::from(auth_token.as_str());

    let api = api_routes()
        .route_layer(middleware::from_fn_with_state(token, require_bearer))
        .with_state(Arc::new(state));

    let meta = Router::new()
        .route("/health", get(health::handle))
        .route("/version", get(version::handle))
        .with_state(VersionInfo {
            build: build_version.into(),
            protocol: crate::PROTOCOL_VERSION,
            min_protocol: crate::MIN_PROTOCOL_VERSION,
        });

    api.merge(meta)
}

fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/transcribe", post(transcribe::handle))
        .route("/api/v1/jobs", post(jobs::submit))
        .route("/api/v1/jobs/:id", get(jobs::status))
        .route("/api/v1/protocols", post(protocols::generate))
        .route("/api/v1/recordings", post(recordings::start))
        .route("/api/v1/recordings/:id/stop", post(recordings::stop))
        .route("/api/v1/meetings", get(meetings::list))
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
    use std::path::PathBuf;
    use tower::ServiceExt;
    use meeting_core::fakes::{
        FakeAudioCapture, FakeJobRepo, FakeLlmProvider, FakeMeetingRepo, FakeTemplateLoader,
        FakeTranscriber,
    };

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn server() -> Router {
        create_server_router(
            AppState {
                transcriber: FakeTranscriber::new("fake"),
                meeting_repo: FakeMeetingRepo::new(),
                job_repo: FakeJobRepo::new(),
                llm: FakeLlmProvider::new(""),
                templates: FakeTemplateLoader::empty(),
                audio_capture: FakeAudioCapture::new(),
                recordings_dir: PathBuf::from("/tmp"),
            },
            TOKEN.to_string(),
            "0.1.0-test",
        )
    }

    // The 7 API routes the contract requires to be auth-gated.
    const API_ROUTES: &[(&str, &str)] = &[
        ("POST", "/api/v1/transcribe"),
        ("POST", "/api/v1/jobs"),
        ("GET", "/api/v1/jobs/abc"),
        ("POST", "/api/v1/protocols"),
        ("POST", "/api/v1/recordings"),
        ("POST", "/api/v1/recordings/abc/stop"),
        ("GET", "/api/v1/meetings"),
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
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "ok");
    }

    #[tokio::test]
    async fn version_needs_no_auth_and_carries_protocol_range() {
        let resp = server()
            .oneshot(Request::builder().uri("/version").body(Body::empty()).unwrap())
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
