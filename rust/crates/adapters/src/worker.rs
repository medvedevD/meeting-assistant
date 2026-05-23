use dashmap::DashMap;
use meeting_core::{
    entities::{meeting::now_unix, ErrorClass, JobProgress, Meeting, PipelineStage},
    ports::{
        JobRepo, LlmProvider, MeetingFileStore, MeetingRepo, ProgressSink, TemplateLoader,
        Transcriber,
    },
    usecases::generate_protocol,
    CoreError,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

const MAX_ATTEMPTS: u32 = 5;
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Shared, in-memory live-progress table keyed by job id. Never persisted
/// (decision #11); the `GET /jobs/:id` handler merges it with the DB row.
pub type LiveProgress = Arc<DashMap<String, JobProgress>>;

/// RU sub-status shown under each pipeline stage.
fn stage_sub(stage: PipelineStage) -> &'static str {
    match stage {
        PipelineStage::Queued => "В очереди",
        PipelineStage::LoadingModel => "Загрузка модели",
        PipelineStage::DecodingAudio => "Декодирование аудио",
        PipelineStage::Transcribing => "Распознавание речи",
        PipelineStage::WritingTranscript => "Сохранение транскрипта",
        PipelineStage::GeneratingProtocol => "Генерация протокола",
        PipelineStage::Done => "Готово",
    }
}

pub struct Worker {
    job_repo: Arc<dyn JobRepo>,
    meeting_repo: Arc<dyn MeetingRepo>,
    transcriber: Arc<dyn Transcriber>,
    file_store: Arc<dyn MeetingFileStore>,
    llm: Arc<dyn LlmProvider>,
    templates: Arc<dyn TemplateLoader>,
    progress: LiveProgress,
}

impl Worker {
    pub fn new(
        job_repo: Arc<dyn JobRepo>,
        meeting_repo: Arc<dyn MeetingRepo>,
        transcriber: Arc<dyn Transcriber>,
        file_store: Arc<dyn MeetingFileStore>,
        llm: Arc<dyn LlmProvider>,
        templates: Arc<dyn TemplateLoader>,
        progress: LiveProgress,
    ) -> Self {
        Self {
            job_repo,
            meeting_repo,
            transcriber,
            file_store,
            llm,
            templates,
            progress,
        }
    }

    /// Publish a coarse stage (percent 0) to the live-progress table.
    fn set_stage(&self, job_id: &str, stage: PipelineStage) {
        self.progress.insert(
            job_id.to_string(),
            JobProgress::new(stage, stage_sub(stage), 0),
        );
    }

    /// Drop the live-progress entry once a job reaches a terminal state; the
    /// `GET /jobs/:id` handler then reports the persisted DB state.
    fn clear_progress(&self, job_id: &str) {
        self.progress.remove(job_id);
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
                let _ = self
                    .job_repo
                    .mark_permanently_failed(
                        &job.id,
                        "meeting not found",
                        None,
                        job.attempts + 1,
                        now,
                    )
                    .await;
                self.clear_progress(&job.id);
                return;
            }
            Err(e) => {
                self.handle_failure(&job, &e).await;
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
                self.clear_progress(&job.id);
            }
            Err(e) => self.handle_failure(&job, &e).await,
        }
    }

    /// Build a progress sink that publishes the transcriber's stage/percent
    /// reports into the live-progress table under this job's id.
    fn transcribe_sink(&self, job_id: &str) -> ProgressSink {
        let map = Arc::clone(&self.progress);
        let id = job_id.to_string();
        Arc::new(move |stage: PipelineStage, percent: u8| {
            map.insert(
                id.clone(),
                JobProgress::new(stage, stage_sub(stage), percent),
            );
        })
    }

    /// Transcribe (or re-transcribe) the meeting's audio and persist the result.
    async fn run_transcribe(
        &self,
        job: &meeting_core::entities::Job,
        meeting: &Meeting,
    ) -> Result<(), meeting_core::CoreError> {
        self.set_stage(&job.id, PipelineStage::LoadingModel);
        let sink = self.transcribe_sink(&job.id);
        let transcript = self
            .transcriber
            .transcribe_with_progress(&meeting.audio_path, sink)
            .await?;
        self.set_stage(&job.id, PipelineStage::WritingTranscript);
        match self
            .file_store
            .write_transcript(&meeting.meeting_dir, &transcript.text)
            .await
        {
            Ok(path) => {
                if let Err(e) = self
                    .meeting_repo
                    .save_transcript_file(&meeting.id, &transcript.text, &path)
                    .await
                {
                    warn!(job_id = %job.id, "save_transcript_file failed: {e}");
                }
            }
            Err(e) => {
                warn!(job_id = %job.id, "write transcript.md failed: {e}");
                if let Err(e) = self
                    .meeting_repo
                    .save_transcript(&meeting.id, &transcript.text)
                    .await
                {
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
        self.set_stage(&job.id, PipelineStage::GeneratingProtocol);
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

        match self
            .file_store
            .write_protocol(&meeting.meeting_dir, &protocol.markdown)
            .await
        {
            Ok(path) => {
                if let Err(e) = self
                    .meeting_repo
                    .save_protocol_file(&meeting.id, &protocol.markdown, &path)
                    .await
                {
                    warn!(job_id = %job.id, "save_protocol_file failed: {e}");
                }
            }
            Err(e) => {
                warn!(job_id = %job.id, "write protocol.md failed: {e}");
                if let Err(e) = self
                    .meeting_repo
                    .save_protocol(&meeting.id, &protocol.markdown)
                    .await
                {
                    warn!(job_id = %job.id, "save_protocol fallback failed: {e}");
                }
            }
        }
        Ok(())
    }

    async fn handle_failure(&self, job: &meeting_core::entities::Job, error: &CoreError) {
        let attempts = job.attempts + 1;
        let now = now_unix();
        let msg = error.to_string();

        if attempts >= MAX_ATTEMPTS {
            // Persist the classified cause for the UI (only on terminal failure).
            let class = ErrorClass::from_core_error(error);
            warn!(job_id = %job.id, attempts, error_class = class.as_str(), "job permanently failed: {msg}");
            let _ = self
                .job_repo
                .mark_permanently_failed(&job.id, &msg, Some(class.as_str()), attempts, now)
                .await;
            self.clear_progress(&job.id);
        } else {
            let backoff_secs = 10i64 * (1 << attempts.min(10));
            let retry_after = now + backoff_secs;
            warn!(job_id = %job.id, attempts, backoff_secs, "job failed, will retry");
            let _ = self
                .job_repo
                .reset_for_retry(&job.id, &msg, attempts, retry_after, now)
                .await;
            // Re-queued; live progress will be re-established on next claim.
            self.clear_progress(&job.id);
        }
    }
}
