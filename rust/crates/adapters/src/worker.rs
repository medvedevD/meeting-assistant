use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};
use meeting_core::{
    entities::{meeting::now_unix, Meeting},
    ports::{JobRepo, LlmProvider, MeetingFileStore, MeetingRepo, TemplateLoader, Transcriber},
    usecases::generate_protocol,
};

const MAX_ATTEMPTS: u32 = 5;
const POLL_INTERVAL: Duration = Duration::from_secs(2);

pub struct Worker {
    job_repo: Arc<dyn JobRepo>,
    meeting_repo: Arc<dyn MeetingRepo>,
    transcriber: Arc<dyn Transcriber>,
    file_store: Arc<dyn MeetingFileStore>,
    llm: Arc<dyn LlmProvider>,
    templates: Arc<dyn TemplateLoader>,
}

impl Worker {
    pub fn new(
        job_repo: Arc<dyn JobRepo>,
        meeting_repo: Arc<dyn MeetingRepo>,
        transcriber: Arc<dyn Transcriber>,
        file_store: Arc<dyn MeetingFileStore>,
        llm: Arc<dyn LlmProvider>,
        templates: Arc<dyn TemplateLoader>,
    ) -> Self {
        Self { job_repo, meeting_repo, transcriber, file_store, llm, templates }
    }

    pub async fn run(self, mut shutdown: tokio::sync::oneshot::Receiver<()>) {
        info!("worker started");

        // Recover any jobs that were `running` when the previous process was killed.
        let now = now_unix();
        match self.job_repo.recover_running_jobs(now).await {
            Ok(0) => {}
            Ok(n) => info!(count = n, "recovered interrupted jobs from previous run"),
            Err(e) => error!("recover_running_jobs failed: {e}"),
        }

        loop {
            // Check for graceful-shutdown signal before claiming the next job.
            match shutdown.try_recv() {
                Ok(()) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    info!("worker shutting down gracefully");
                    break;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
            }

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
        info!("worker stopped");
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

        let result = if job.kind.is_transcription() {
            self.run_transcribe(&job, &meeting).await
        } else {
            // JobKind::RegenerateProtocol
            self.run_regenerate_protocol(&job, &meeting).await
        };

        match result {
            Ok(()) => {
                let now = now_unix();
                if let Err(e) = self.job_repo.mark_done(&job.id, now).await {
                    error!(job_id = %job.id, "mark_done failed: {e}");
                } else {
                    info!(job_id = %job.id, kind = %job.kind.as_str(), "job done");
                }
            }
            Err(e) => self.handle_failure(&job, &e.to_string()).await,
        }
    }

    /// Transcribe (or re-transcribe) the meeting's audio and persist the result.
    async fn run_transcribe(
        &self,
        job: &meeting_core::entities::Job,
        meeting: &Meeting,
    ) -> Result<(), meeting_core::CoreError> {
        let transcript = self.transcriber.transcribe(&meeting.audio_path).await?;
        match self.file_store.write_transcript(&meeting.meeting_dir, &transcript.text).await {
            Ok(path) => {
                if let Err(e) = self.meeting_repo
                    .save_transcript_file(&meeting.id, &transcript.text, &path)
                    .await
                {
                    warn!(job_id = %job.id, "save_transcript_file failed: {e}");
                }
            }
            Err(e) => {
                warn!(job_id = %job.id, "write transcript.md failed: {e}");
                if let Err(e) = self.meeting_repo.save_transcript(&meeting.id, &transcript.text).await {
                    warn!(job_id = %job.id, "save_transcript fallback failed: {e}");
                }
            }
        }
        Ok(())
    }

    /// Regenerate the protocol from the stored transcript via the LLM.
    async fn run_regenerate_protocol(
        &self,
        job: &meeting_core::entities::Job,
        meeting: &Meeting,
    ) -> Result<(), meeting_core::CoreError> {
        use meeting_core::CoreError;
        let transcript = meeting
            .transcript_text
            .as_deref()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| CoreError::Validation("meeting has no transcript".into()))?;

        let protocol = generate_protocol(
            Arc::clone(&self.llm),
            Arc::clone(&self.templates),
            transcript,
            job.template_name.as_deref(),
            Some(&meeting.name),
        )
        .await?;

        match self.file_store.write_protocol(&meeting.meeting_dir, &protocol.markdown).await {
            Ok(path) => {
                if let Err(e) = self.meeting_repo
                    .save_protocol_file(&meeting.id, &protocol.markdown, &path)
                    .await
                {
                    warn!(job_id = %job.id, "save_protocol_file failed: {e}");
                }
            }
            Err(e) => {
                warn!(job_id = %job.id, "write protocol.md failed: {e}");
                if let Err(e) = self.meeting_repo.save_protocol(&meeting.id, &protocol.markdown).await {
                    warn!(job_id = %job.id, "save_protocol fallback failed: {e}");
                }
            }
        }
        Ok(())
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
