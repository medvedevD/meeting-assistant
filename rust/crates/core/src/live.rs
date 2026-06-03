use crate::entities::JobProgress;
use dashmap::DashMap;
use std::sync::Arc;

/// Live, in-memory job-progress table keyed by job id. Shared between the
/// worker (writer) and the `GET /jobs/:id` / `GET /active-jobs` handlers
/// (readers). Never persisted — see `plans/done/active-jobs-store`.
pub type LiveProgress = Arc<DashMap<String, JobProgress>>;
