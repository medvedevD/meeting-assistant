use crate::{
    entities::{Job, JobKind},
    CoreError,
};
use async_trait::async_trait;

/// All `JobKind`s. Used by [`JobRepo::claim_pending`] as the unfiltered
/// shorthand and by tests that don't care which kind they claim.
pub const ALL_JOB_KINDS: &[JobKind] = &[
    JobKind::Transcribe,
    JobKind::ReprocessTranscribe,
    JobKind::RegenerateProtocol,
];

#[async_trait]
pub trait JobRepo: Send + Sync {
    async fn enqueue(&self, job: &Job) -> Result<(), CoreError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Job>, CoreError>;
    /// List in-flight jobs (`pending` or `running`), oldest first. Used to seed
    /// the UI's active-jobs view after an app restart.
    async fn list_active(&self) -> Result<Vec<Job>, CoreError>;
    /// Atomically claim one pending job whose `kind` is in `kinds` and whose
    /// `retry_after <= now_ts`. The split worker-pool architecture
    /// ([`plans/active/worker-concurrency-pool`]) uses this to run a
    /// CPU-bound transcribe worker and an IO-bound protocol worker that each
    /// claim disjoint kinds.
    async fn claim_pending_kind(
        &self,
        kinds: &[JobKind],
        now_ts: i64,
    ) -> Result<Option<Job>, CoreError>;
    /// Convenience wrapper that claims any kind. Used by tests; production
    /// workers call [`Self::claim_pending_kind`] with their disjoint filter.
    async fn claim_pending(&self, now_ts: i64) -> Result<Option<Job>, CoreError> {
        self.claim_pending_kind(ALL_JOB_KINDS, now_ts).await
    }
    async fn mark_done(&self, id: &str, now_ts: i64) -> Result<(), CoreError>;
    /// Reset job to pending for a future retry attempt.
    async fn reset_for_retry(
        &self,
        id: &str,
        error: &str,
        attempts: u32,
        retry_after: i64,
        now_ts: i64,
    ) -> Result<(), CoreError>;
    /// Permanently fail the job (max attempts exhausted). `error_class` is the
    /// classified cause persisted for the UI (decision #11); `None` when the
    /// cause is not classifiable.
    async fn mark_permanently_failed(
        &self,
        id: &str,
        error: &str,
        error_class: Option<&str>,
        attempts: u32,
        now_ts: i64,
    ) -> Result<(), CoreError>;

    /// Reset any jobs stuck in `running` state back to `pending`.
    ///
    /// Called once at worker startup to recover jobs that were interrupted by
    /// a previous process crash (e.g. `kill -9`). Returns the number of recovered jobs.
    async fn recover_running_jobs(&self, now_ts: i64) -> Result<u64, CoreError>;

    /// Atomically transition a `pending` job to terminal `failed` with
    /// `error_class='cancelled'`. Returns the number of rows actually changed:
    /// 0 if the job was already non-pending (idempotent), 1 on success.
    ///
    /// Used by `DELETE /api/v1/jobs/:id` when the job has not yet been
    /// claimed by a worker. See `plans/active/job-cancellation`.
    async fn cancel_pending(&self, id: &str, now_ts: i64) -> Result<u64, CoreError>;

    /// Mark a `pending`-or-`running` job as terminal `failed` with
    /// `error_class='cancelled'`. Used by the worker after observing the
    /// cancellation token at a safe checkpoint. Guarded by
    /// `status IN ('pending','running')` so a race with `mark_done` /
    /// `mark_permanently_failed` is a no-op (returns 0).
    async fn mark_cancelled(&self, id: &str, now_ts: i64) -> Result<u64, CoreError>;
}
