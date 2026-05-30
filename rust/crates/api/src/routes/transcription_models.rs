use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    InstallStarted, InstallationView, ModelServiceError, TranscriptionModelService,
    TranscriptionModelsView,
};

type Svc = Arc<dyn TranscriptionModelService>;

pub async fn list(
    State(svc): State<Svc>,
) -> Result<Json<TranscriptionModelsView>, (StatusCode, String)> {
    svc.list().await.map(Json).map_err(error_response)
}

pub async fn install(
    State(svc): State<Svc>,
    Path(id): Path<String>,
) -> Result<Json<InstallStarted>, (StatusCode, String)> {
    svc.start_install(id)
        .await
        .map(Json)
        .map_err(error_response)
}

pub async fn installation(
    State(svc): State<Svc>,
    Path(job_id): Path<String>,
) -> Result<Json<InstallationView>, (StatusCode, String)> {
    svc.installation(job_id)
        .await
        .map(Json)
        .map_err(error_response)
}

pub async fn delete(
    State(svc): State<Svc>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    svc.delete_model(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(error_response)
}

fn error_response(error: ModelServiceError) -> (StatusCode, String) {
    let status = match error {
        ModelServiceError::BadRequest(_) => StatusCode::BAD_REQUEST,
        ModelServiceError::NotFound(_) => StatusCode::NOT_FOUND,
        ModelServiceError::Conflict(_) => StatusCode::CONFLICT,
        ModelServiceError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, error.message().to_string())
}
