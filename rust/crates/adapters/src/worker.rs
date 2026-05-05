use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};
use meeting_core::{
    entities::meeting::now_unix,
    ports::{JobRepo, MeetingRepo, Transcriber},
};

const MAX_ATTEMPTS: u32 = 5;
const POLL_INTERVAL: Duration = Duration::from_secs(2);

pub struct Worker {
    job_repo: Arc<dyn JobRepo>,
    meeting_repo: Arc<dyn MeetingRepo>,
    transcriber: Arc<dyn Transcriber>,
}

impl Worker {
    pub fn new(
        job_repo: Arc<dyn JobRepo>,
        meeting_repo: Arc<dyn MeetingRepo>,
        transcriber: Arc<dyn Transcriber>,
    ) -> Self {
        Self { job_repo, meeting_repo, transcriber }
    }

    pub async fn run(self) {
        info!("worker started");
        loop {
            let now = now_unix();
            match self.job_repo.claim_pending(now).await {
                Ok(Some(job)) => {
                    info!(job_id = %job.id, kind = %job.kind.as_str(), "claimed job");
                    self.execute(job).await;
                }
                Ok(None) => {
                    sleep(POLL_INTERVAL).await;
                }
                Err(e) => {
                    error!("job_repo.claim_pending error: {e}");
                    sleep(POLL_INTERVAL).await;
                }
            }
        }
    }

    async fn execute(&self, job: meeting_core::entities::Job) {
        let meeting = match self.meeting_repo.find_by_id(&job.meeting_id).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                let now = now_unix();
                error!(job_id = %job.id, "meeting not found — failing permanently");
                let _ = self.job_repo.mark_permanently_failed(
                    &job.id, "meeting not found", job.attempts + 1, now,
                ).await;
                return;
            }
            Err(e) => {
                self.handle_failure(&job, &e.to_string()).await;
                return;
            }
        };

        match self.transcriber.transcribe(&meeting.audio_path).await {
            Ok(transcript) => {
                let now = now_unix();
                if let Err(e) = self.meeting_repo.save_transcript(&meeting.id, &transcript.text).await {
                    warn!(job_id = %job.id, "save_transcript failed: {e}");
                }
                if let Err(e) = self.job_repo.mark_done(&job.id, now).await {
                    error!(job_id = %job.id, "mark_done failed: {e}");
                } else {
                    info!(job_id = %job.id, "job done");
                }
            }
            Err(e) => {
                self.handle_failure(&job, &e.to_string()).await;
            }
        }
    }

    async fn handle_failure(&self, job: &meeting_core::entities::Job, error: &str) {
        let attempts = job.attempts + 1;
        let now = now_unix();

        if attempts >= MAX_ATTEMPTS {
            warn!(job_id = %job.id, attempts, "job permanently failed: {error}");
            let _ = self.job_repo.mark_permanently_failed(&job.id, error, attempts, now).await;
        } else {
            let backoff_secs = 10i64 * (1 << attempts.min(10));
            let retry_after = now + backoff_secs;
            warn!(job_id = %job.id, attempts, backoff_secs, "job failed, will retry");
            let _ = self.job_repo.reset_for_retry(&job.id, error, attempts, retry_after, now).await;
        }
    }
}
