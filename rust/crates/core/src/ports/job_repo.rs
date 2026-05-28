use crate::{entities::Job, CoreError};
use async_trait::async_trait;

#[async_trait]
pub trait JobRepo: Send + Sync {
    async fn enqueue(&self, job: &Job) -> Result<(), CoreError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Job>, CoreError>;
    /// List in-flight jobs (`pending` or `running`), oldest first. Used to seed
    /// the UI's active-jobs view after an app restart.
    async fn list_active(&self) -> Result<Vec<Job>, CoreError>;
    /// Atomically claim one pending job whose retry_after <= now_ts.
    async fn claim_pending(&self, now_ts: i64) -> Result<Option<Job>, CoreError>;
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
}
