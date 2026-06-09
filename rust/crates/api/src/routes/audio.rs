use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use meeting_core::ports::{
    AudioDeviceEnumerator, AudioDeviceList, AudioLevel, AudioLevelMonitor, CaptureSource,
    CaptureSpec, ResolvedDevices,
};
use serde::{Deserialize, Serialize};

/// State for the `/api/v1/audio/*` routes: device enumeration + the live level
/// monitor used by the settings device test.
pub struct AudioApi {
    pub enumerator: Arc<dyn AudioDeviceEnumerator>,
    pub monitor: Arc<dyn AudioLevelMonitor>,
}

type Svc = Arc<AudioApi>;

/// `GET /api/v1/audio/devices` — selectable mic inputs and system-audio sources.
pub async fn devices(State(svc): State<Svc>) -> Result<Json<AudioDeviceList>, (StatusCode, String)> {
    svc.enumerator
        .list_devices()
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

#[derive(Deserialize)]
pub struct MonitorRequest {
    /// Client-minted opaque session id (the GUI polls + stops by this id).
    pub id: String,
    #[serde(default)]
    pub source: CaptureSource,
    #[serde(default)]
    pub mic_device: Option<String>,
    #[serde(default)]
    pub system_device: Option<String>,
}

#[derive(Serialize)]
pub struct MonitorResponse {
    pub resolved: ResolvedDevices,
}

/// `POST /api/v1/audio/monitor` — start metering one device for the test meter.
pub async fn monitor_start(
    State(svc): State<Svc>,
    Json(req): Json<MonitorRequest>,
) -> Result<Json<MonitorResponse>, (StatusCode, String)> {
    let spec = CaptureSpec {
        source: req.source,
        echo_cancel: false,
        mic_device: req.mic_device,
        system_device: req.system_device,
    };
    svc.monitor
        .start(&req.id, spec)
        .await
        .map(|resolved| Json(MonitorResponse { resolved }))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// `GET /api/v1/audio/monitor/:id` — the latest input level for a session.
pub async fn monitor_level(
    State(svc): State<Svc>,
    Path(id): Path<String>,
) -> Result<Json<AudioLevel>, StatusCode> {
    svc.monitor.level(&id).map(Json).ok_or(StatusCode::NOT_FOUND)
}

/// `DELETE /api/v1/audio/monitor/:id` — stop a metering session.
pub async fn monitor_stop(
    State(svc): State<Svc>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.monitor
        .stop(&id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::Request,
        routing::{get, post},
        Router,
    };
    use http_body_util::BodyExt;
    use meeting_core::{
        fakes::{FakeAudioDeviceEnumerator, FakeAudioLevelMonitor},
        ports::{AudioDevice, AudioDeviceList},
    };
    use tower::ServiceExt;

    fn app() -> Router {
        let svc = Arc::new(AudioApi {
            enumerator: FakeAudioDeviceEnumerator::new(),
            monitor: FakeAudioLevelMonitor::new(),
        });
        Router::new()
            .route("/api/v1/audio/devices", get(devices))
            .route("/api/v1/audio/monitor", post(monitor_start))
            .route(
                "/api/v1/audio/monitor/:id",
                get(monitor_level).delete(monitor_stop),
            )
            .with_state(svc)
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn lists_devices_with_input_output_and_selectable_flag() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audio/devices")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert!(!json["input"].as_array().unwrap().is_empty());
        assert!(!json["output"].as_array().unwrap().is_empty());
        assert_eq!(json["system_selectable"], true);
        assert_eq!(json["input"][0]["is_default"], true);
    }

    #[tokio::test]
    async fn devices_reports_system_not_selectable_when_no_outputs() {
        let svc = Arc::new(AudioApi {
            enumerator: FakeAudioDeviceEnumerator::with_list(AudioDeviceList {
                input: vec![AudioDevice {
                    id: "m".into(),
                    label: "m".into(),
                    is_default: true,
                }],
                output: vec![],
                system_selectable: false,
            }),
            monitor: FakeAudioLevelMonitor::new(),
        });
        let app = Router::new()
            .route("/api/v1/audio/devices", get(devices))
            .with_state(svc);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audio/devices")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = body_json(response).await;
        assert_eq!(json["system_selectable"], false);
        assert_eq!(json["output"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn monitor_start_then_level_then_stop() {
        let app = app();

        let start = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/audio/monitor")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "id": "t1", "source": "mic" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(start.status(), StatusCode::OK);

        let level = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audio/monitor/t1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(level.status(), StatusCode::OK);
        let json = body_json(level).await;
        assert!(json["level"].as_f64().is_some());

        let stop = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/audio/monitor/t1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stop.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn level_for_unknown_session_is_404() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audio/monitor/nope")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
