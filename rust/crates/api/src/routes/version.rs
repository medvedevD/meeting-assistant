use axum::{extract::State, Json};
use crate::router::VersionInfo;

/// Version / compatibility probe. **No auth.** Carries the informational build
/// version plus the protocol-compat range. The client proceeds iff its protocol
/// version is within `[min_protocol ..= protocol]`.
pub async fn handle(State(info): State<VersionInfo>) -> Json<VersionInfo> {
    Json(info)
}
