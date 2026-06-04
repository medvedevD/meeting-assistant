use crate::entities::JobProgress;
use dashmap::DashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// One entry in the live-jobs table: the in-flight `JobProgress` snapshot and
/// the cancellation token the worker checks at every safe checkpoint. Both
/// fields share an identical lifetime — created when the worker claims a job,
/// dropped from the map when the worker reaches a terminal state. See
/// `plans/active/job-cancellation` (Decision #2).
#[derive(Clone)]
pub struct LiveEntry {
    pub progress: JobProgress,
    pub cancel: CancellationToken,
}

/// Live, in-memory job table keyed by job id. Shared between the worker
/// (writer) and the `GET /jobs/:id`, `GET /active-jobs`, and
/// `DELETE /jobs/:id` handlers (readers / cancel-triggers). Never persisted.
pub type LiveJobs = Arc<DashMap<String, LiveEntry>>;
