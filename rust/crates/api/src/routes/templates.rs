use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::template_service::{TemplateError, TemplateNotice, TemplateService};

type Svc = Arc<dyn TemplateService>;

fn err(e: TemplateError) -> (StatusCode, String) {
    (e.status(), e.message())
}

/// `GET /api/v1/templates` — all templates with bodies (for list + preview).
pub async fn list(State(svc): State<Svc>) -> Result<Json<Value>, (StatusCode, String)> {
    let items = svc.list().await.map_err(err)?;
    Ok(Json(json!({ "templates": items })))
}

/// `GET /api/v1/templates/:name` — one template's body.
pub async fn get(
    State(svc): State<Svc>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let body = svc.get(&name).await.map_err(err)?;
    Ok(Json(json!({ "name": name, "body": body })))
}

#[derive(Deserialize)]
pub struct SaveRequest {
    pub body: String,
}

/// `PUT /api/v1/templates/:name` — create or overwrite a template.
pub async fn put(
    State(svc): State<Svc>,
    Path(name): Path<String>,
    Json(req): Json<SaveRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.save(&name, &req.body).await.map_err(err)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/v1/templates/:name` — delete a template. May return a notice (e.g.
/// the template was the configured default, now cleared): `warning_code` is what
/// the GUI localizes, `warning_detail` its raw cause, `warning` an English sentence.
pub async fn delete(
    State(svc): State<Svc>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let notice = svc.delete(&name).await.map_err(err)?;
    Ok(Json(json!({
        "deleted": name,
        "warning": notice.as_ref().map(TemplateNotice::message),
        "warning_code": notice.as_ref().map(TemplateNotice::code),
        "warning_detail": notice.as_ref().and_then(TemplateNotice::detail),
    })))
}

#[derive(Deserialize)]
pub struct RenameRequest {
    pub new_name: String,
}

/// `POST /api/v1/templates/:name/rename` — rename a template.
pub async fn rename(
    State(svc): State<Svc>,
    Path(name): Path<String>,
    Json(req): Json<RenameRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.rename(&name, &req.new_name).await.map_err(err)?;
    Ok(StatusCode::NO_CONTENT)
}
