use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{SetupStatus, SetupStatusService};

/// `GET /api/v1/setup-status`
pub async fn get(State(svc): State<Arc<dyn SetupStatusService>>) -> Json<SetupStatus> {
    Json(svc.status())
}
