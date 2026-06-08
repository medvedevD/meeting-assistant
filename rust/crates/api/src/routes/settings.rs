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

#[derive(Deserialize)]
pub struct PassphraseRequest {
    pub passphrase: String,
}

#[derive(Deserialize)]
pub struct ChangePassphraseRequest {
    pub old: String,
    pub new: String,
}

/// `POST /api/v1/settings/secret-store/protect` — encrypt the plaintext
/// fallback into a passphrase vault and leave it unlocked.
pub async fn protect_secret_store(
    State(svc): State<Svc>,
    Json(req): Json<PassphraseRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.protect_secret_store(req.passphrase)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

/// `POST /api/v1/settings/secret-store/unlock` — decrypt the vault. A wrong
/// passphrase yields `401`.
pub async fn unlock_secret_store(
    State(svc): State<Svc>,
    Json(req): Json<PassphraseRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.unlock_secret_store(req.passphrase)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))
}

/// `POST /api/v1/settings/secret-store/lock` — drop decrypted keys from memory.
pub async fn lock_secret_store(
    State(svc): State<Svc>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.lock_secret_store()
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

/// `POST /api/v1/settings/secret-store/passphrase` — re-encrypt under a new
/// passphrase. A wrong `old` yields `401`.
pub async fn change_secret_passphrase(
    State(svc): State<Svc>,
    Json(req): Json<ChangePassphraseRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.change_secret_passphrase(req.old, req.new)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))
}

/// `POST /api/v1/settings/secret-store/migrate` — import an orphaned vault into
/// the keystore. A wrong passphrase yields `401`.
pub async fn migrate_secret_store(
    State(svc): State<Svc>,
    Json(req): Json<PassphraseRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.migrate_secret_store_to_keyring(req.passphrase)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))
}

/// `POST /api/v1/settings/secret-store/reset` — discard the store (keys lost).
pub async fn reset_secret_store(
    State(svc): State<Svc>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.reset_secret_store()
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}
