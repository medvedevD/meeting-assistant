use crate::{
    entities::{meeting::now_unix, JobStatus},
    ports::JobRepo,
    CoreError, LiveJobs,
};
use std::sync::Arc;

/// Result of a `DELETE /api/v1/jobs/:id`. Mapped to HTTP status codes by the
/// API handler (see `plans/active/job-cancellation` ADR-004):
///
/// - `Cancelled` / `Cancelling` → `202 Accepted`
/// - `AlreadyTerminal` → `204 No Content`
/// - `NotFound` → `404 Not Found`
#[derive(Debug, PartialEq, Eq)]
pub enum CancelOutcome {
    /// Job was running; the cancellation token was flipped. The worker will
    /// observe it at the next safe checkpoint and persist the terminal state.
    Cancelling,
    /// Job was pending and has been transitioned to terminal `failed`
    /// (`error_class='cancelled'`) in a single repo write.
    Cancelled,
    /// Job already reached a terminal state — no action taken.
    AlreadyTerminal,
    /// No job with the given id exists.
    NotFound,
}

/// Cooperative cancellation for a single job. For a pending job, the repo
/// flips the row to terminal `failed` directly; for a running job, the
/// in-memory cancellation token is signalled and the worker drives the
/// terminal transition at its next checkpoint.
pub async fn cancel_job(
    repo: Arc<dyn JobRepo>,
    live: LiveJobs,
    id: &str,
) -> Result<CancelOutcome, CoreError> {
    let Some(job) = repo.find_by_id(id).await? else {
        return Ok(CancelOutcome::NotFound);
    };
    match job.status {
        JobStatus::Pending => {
            let n = repo.cancel_pending(id, now_unix()).await?;
            if n == 0 {
                // Raced with a concurrent claim — the row is no longer
                // pending. Treat as "already moved on"; the caller can retry
                // a DELETE if the new state is `running`.
                Ok(CancelOutcome::AlreadyTerminal)
            } else {
                Ok(CancelOutcome::Cancelled)
            }
        }
        JobStatus::Running => {
            // The worker inserts a `LiveEntry` *before* spawning the per-job
            // task, so a `running` row without an entry is a microsecond race
            // window between `claim_pending_kind` and the insert. We still
            // report `Cancelling` — the caller can retry; subsequent DELETEs
            // are cheap and idempotent.
            if let Some(entry) = live.get(id) {
                entry.cancel.cancel();
            }
            Ok(CancelOutcome::Cancelling)
        }
        JobStatus::Done | JobStatus::Failed => Ok(CancelOutcome::AlreadyTerminal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{ErrorClass, Job, JobProgress, PipelineStage};
    use crate::fakes::FakeJobRepo;
    use crate::{LiveEntry, LiveProgress};
    use tokio_util::sync::CancellationToken;

    fn empty_live() -> LiveJobs {
        LiveProgress::new()
    }

    #[tokio::test]
    async fn cancel_unknown_returns_not_found() {
        let jr = FakeJobRepo::new();
        let out = cancel_job(jr as Arc<dyn JobRepo>, empty_live(), "ghost")
            .await
            .unwrap();
        assert_eq!(out, CancelOutcome::NotFound);
    }

    #[tokio::test]
    async fn cancel_pending_returns_cancelled_and_persists_state() {
        let jr = FakeJobRepo::new();
        let job = Job::new_transcribe("m1".into());
        jr.enqueue(&job).await.unwrap();

        let out = cancel_job(Arc::clone(&jr) as Arc<dyn JobRepo>, empty_live(), &job.id)
            .await
            .unwrap();
        assert_eq!(out, CancelOutcome::Cancelled);

        let in_db = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(in_db.status, JobStatus::Failed);
        assert_eq!(in_db.error_class, Some(ErrorClass::Cancelled));
    }

    #[tokio::test]
    async fn cancel_running_returns_cancelling_and_signals_token() {
        let jr = FakeJobRepo::new();
        let job = Job::new_transcribe("m1".into());
        jr.enqueue(&job).await.unwrap();
        // Move to running via claim.
        let claimed = jr.claim_pending(i64::MAX).await.unwrap().unwrap();
        assert_eq!(claimed.status, JobStatus::Running);

        // Worker would normally insert this entry before spawning execute.
        let live: LiveJobs = empty_live();
        let token = CancellationToken::new();
        live.insert(
            job.id.clone(),
            LiveEntry {
                progress: JobProgress::new(PipelineStage::Queued, "В очереди", 0),
                cancel: token.clone(),
            },
        );

        let out = cancel_job(Arc::clone(&jr) as Arc<dyn JobRepo>, Arc::clone(&live), &job.id)
            .await
            .unwrap();
        assert_eq!(out, CancelOutcome::Cancelling);
        assert!(token.is_cancelled(), "token should be flipped");

        // The repo state is still `running` — the worker is responsible
        // for marking it cancelled after observing the token.
        let in_db = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(in_db.status, JobStatus::Running);
    }

    #[tokio::test]
    async fn cancel_done_returns_already_terminal() {
        let jr = FakeJobRepo::new();
        let job = Job::new_transcribe("m1".into());
        jr.enqueue(&job).await.unwrap();
        jr.mark_done(&job.id, 1).await.unwrap();

        let out = cancel_job(Arc::clone(&jr) as Arc<dyn JobRepo>, empty_live(), &job.id)
            .await
            .unwrap();
        assert_eq!(out, CancelOutcome::AlreadyTerminal);
    }

    #[tokio::test]
    async fn cancel_failed_returns_already_terminal() {
        let jr = FakeJobRepo::new();
        let job = Job::new_transcribe("m1".into());
        jr.enqueue(&job).await.unwrap();
        jr.mark_permanently_failed(&job.id, "boom", Some("api_auth"), 5, 1)
            .await
            .unwrap();

        let out = cancel_job(Arc::clone(&jr) as Arc<dyn JobRepo>, empty_live(), &job.id)
            .await
            .unwrap();
        assert_eq!(out, CancelOutcome::AlreadyTerminal);
    }

    #[tokio::test]
    async fn double_cancel_pending_is_idempotent() {
        let jr = FakeJobRepo::new();
        let job = Job::new_transcribe("m1".into());
        jr.enqueue(&job).await.unwrap();

        let first = cancel_job(Arc::clone(&jr) as Arc<dyn JobRepo>, empty_live(), &job.id)
            .await
            .unwrap();
        assert_eq!(first, CancelOutcome::Cancelled);

        let second = cancel_job(Arc::clone(&jr) as Arc<dyn JobRepo>, empty_live(), &job.id)
            .await
            .unwrap();
        assert_eq!(second, CancelOutcome::AlreadyTerminal);
    }

    #[tokio::test]
    async fn cancel_running_without_live_entry_still_returns_cancelling() {
        // Microsecond race between claim and LiveEntry insert: the DB is
        // already `running` but the worker hasn't inserted the token yet.
        // We still report `Cancelling`; the caller can retry.
        let jr = FakeJobRepo::new();
        let job = Job::new_transcribe("m1".into());
        jr.enqueue(&job).await.unwrap();
        jr.claim_pending(i64::MAX).await.unwrap();

        let out = cancel_job(Arc::clone(&jr) as Arc<dyn JobRepo>, empty_live(), &job.id)
            .await
            .unwrap();
        assert_eq!(out, CancelOutcome::Cancelling);
    }
}
