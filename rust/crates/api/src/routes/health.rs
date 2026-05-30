use axum::Json;
use serde_json::{json, Value};

/// Liveness probe. **No auth.** Returns 200 once the router is serving — by
/// construction the worker and adapter graph are already wired before
/// `axum::serve` is called, so a 200 here means the sidecar is usable.
pub async fn handle() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
