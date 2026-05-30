use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::Value;

use crate::settings_service::SettingsService;

type Svc = Arc<dyn SettingsService>;

/// `GET /api/v1/settings` — sanitized current settings (no secrets).
pub async fn get(State(svc): State<Svc>) -> Json<Value> {
    Json(svc.snapshot())
}

/// `PUT /api/v1/settings` — persist + hot-apply a full settings object.
pub async fn put(
    State(svc): State<Svc>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    svc.update(body)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

#[derive(Deserialize)]
pub struct SecretRequest {
    pub provider: String,
    /// `null` deletes the stored key.
    pub value: Option<String>,
}

/// `PUT /api/v1/settings/secret` — store or delete a provider API key.
pub async fn put_secret(
    State(svc): State<Svc>,
    Json(req): Json<SecretRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.set_secret(req.provider, req.value)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

#[derive(Deserialize)]
pub struct TestRequest {
    pub provider: String,
}

/// `POST /api/v1/settings/test` — probe a provider's credentials.
pub async fn test(
    State(svc): State<Svc>,
    Json(req): Json<TestRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.test_provider(req.provider)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))
}
