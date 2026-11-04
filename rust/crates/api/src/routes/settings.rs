use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use meeting_core::{entities::ErrorClass, CoreError};
use serde::Deserialize;
use serde_json::{json, Value};

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
    /// Model to probe instead of the saved one (the unsaved choice on screen).
    #[serde(default)]
    pub model: Option<String>,
}

/// `POST /api/v1/settings/test` — probe a provider's key against a model.
/// A failure carries `{error_class, message}` (the same classes as a failed
/// job) so the UI can say *why*: a bad key, a model outside the tier, quota…
pub async fn test(
    State(svc): State<Svc>,
    Json(req): Json<TestRequest>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    svc.test_provider(req.provider, req.model)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(provider_error)
}

#[derive(Deserialize)]
pub struct ModelsQuery {
    pub provider: String,
}

/// `GET /api/v1/settings/models?provider=…` — models the stored key can use,
/// as `{provider, models: [id…]}`. Fails like `/settings/test` (classified
/// `{error_class, message}`), so the UI can say why it fell back to its
/// built-in suggestions.
pub async fn models(
    State(svc): State<Svc>,
    Query(q): Query<ModelsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let models = svc
        .list_models(q.provider.clone())
        .await
        .map_err(provider_error)?;
    Ok(Json(json!({ "provider": q.provider, "models": models })))
}

/// A failed provider call as `{error_class, message}`: 400 for a bad request
/// from the GUI, 502 for anything the provider (or its absence) caused.
fn provider_error(e: CoreError) -> (StatusCode, Json<Value>) {
    let status = match e {
        CoreError::Validation(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_GATEWAY,
    };
    let body = json!({
        "error_class": ErrorClass::from_core_error(&e).as_str(),
        "message": e.to_string(),
    });
    (status, Json(body))
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
pub async fn lock_secret_store(State(svc): State<Svc>) -> Result<StatusCode, (StatusCode, String)> {
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
