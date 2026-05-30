use dashmap::DashMap;
use meeting_core::{
    entities::{meeting::now_unix, ErrorClass, Job, JobKind, JobProgress, Meeting, PipelineStage},
    ports::{
        JobRepo, LlmProvider, MeetingFileStore, MeetingRepo, ProgressSink, TemplateLoader,
        Transcriber,
    },
    usecases::generate_protocol,
    CoreError,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

const MAX_ATTEMPTS: u32 = 5;
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Shared, in-memory live-progress table keyed by job id. Never persisted
/// (decision #11); the `GET /jobs/:id` handler merges it with the DB row.
pub type LiveProgress = Arc<DashMap<String, JobProgress>>;

/// Kinds the CPU-bound transcribe worker claims.
pub const TRANSCRIBE_KINDS: &[JobKind] = &[JobKind::Transcribe, JobKind::ReprocessTranscribe];
/// Kinds the IO-bound protocol worker claims.
pub const PROTOCOL_KINDS: &[JobKind] = &[JobKind::RegenerateProtocol];

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

/// One half of the split worker pool. Two instances run side-by-side: the
/// transcribe worker claims CPU-bound kinds (`Transcribe` / `ReprocessTranscribe`)
/// bounded by `cpu_pool`; the protocol worker claims `RegenerateProtocol`
/// bounded by `io_pool`. See `plans/active/worker-concurrency-pool` for the
/// architecture decision.
pub struct Worker {
    job_repo: Arc<dyn JobRepo>,
    meeting_repo: Arc<dyn MeetingRepo>,
    transcriber: Arc<dyn Transcriber>,
    file_store: Arc<dyn MeetingFileStore>,
    llm: Arc<dyn LlmProvider>,
    templates: Arc<dyn TemplateLoader>,
    progress: LiveProgress,
    kind_filter: &'static [JobKind],
    permits: Arc<Semaphore>,
    pool_size: usize,
    name: &'static str,
}

impl Worker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        job_repo: Arc<dyn JobRepo>,
        meeting_repo: Arc<dyn MeetingRepo>,
        transcriber: Arc<dyn Transcriber>,
        file_store: Arc<dyn MeetingFileStore>,
        llm: Arc<dyn LlmProvider>,
        templates: Arc<dyn TemplateLoader>,
        progress: LiveProgress,
        kind_filter: &'static [JobKind],
        pool_size: usize,
        name: &'static str,
    ) -> Self {
        let pool_size = pool_size.max(1);
        Self {
            job_repo,
            meeting_repo,
            transcriber,
            file_store,
            llm,
            templates,
            progress,
            kind_filter,
            permits: Arc::new(Semaphore::new(pool_size)),
            pool_size,
            name,
        }
    }

    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    /// Number of currently-executing per-job tasks (held permits).
    pub fn in_flight(&self) -> usize {
        self.pool_size
            .saturating_sub(self.permits.available_permits())
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

    /// Main loop. Permit-before-claim: an idle pool never holds rows in
    /// `running`. Shutdown interrupts both the permit-acquire and the claim
    /// via `tokio::select!`; in-flight tasks are drained best-effort and
    /// any survivors are aborted when the sidecar's outer 1.2s budget
    /// expires (they recover on next start via `recover_running_jobs`).
    pub async fn run(self: Arc<Self>, mut shutdown: tokio::sync::oneshot::Receiver<()>) {
        info!(worker = self.name, pool_size = self.pool_size, "worker started");

        let mut inflight: JoinSet<()> = JoinSet::new();

        loop {
            // 1. Acquire a permit (interruptible by shutdown).
            let permit = tokio::select! {
                biased;
                _ = &mut shutdown => break,
                p = Arc::clone(&self.permits).acquire_owned() => {
                    p.expect("worker semaphore must not be closed")
                }
            };

            // 2. Claim a pending job for this worker's kinds (also interruptible).
            let now = now_unix();
            let claim = tokio::select! {
                biased;
                _ = &mut shutdown => { drop(permit); break }
                r = self.job_repo.claim_pending_kind(self.kind_filter, now) => r,
            };

            match claim {
                Ok(Some(job)) => {
                    info!(
                        worker = self.name,
                        job_id = %job.id,
                        kind = %job.kind.as_str(),
                        "claimed job"
                    );
                    let me = Arc::clone(&self);
                    inflight.spawn(async move {
                        me.execute(job).await;
                        drop(permit);
                    });
                }
                Ok(None) => {
                    drop(permit);
                    sleep(POLL_INTERVAL).await;
                }
                Err(e) => {
                    drop(permit);
                    error!(worker = self.name, "job_repo.claim_pending_kind error: {e}");
                    sleep(POLL_INTERVAL).await;
                }
            }
        }

        // Drain in-flight tasks. The sidecar's outer timeout caps how long
        // this can run; if it expires the run task is aborted and JoinSet's
        // Drop cancels what's left.
        let mut drained = 0u32;
        while inflight.join_next().await.is_some() {
            drained += 1;
        }
        info!(worker = self.name, drained, "worker stopped");
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
                // Backend-owned chain: a transcription job flagged `then_protocol`
                // enqueues the protocol job as soon as the transcript is written,
                // so the generation flow's second step survives a client restart.
                // Enqueue before `mark_done` so the protocol job already exists in
                // the queue by the time a client observes the transcription done.
                if job.kind.is_transcription() && job.then_protocol {
                    let proto =
                        Job::new_regenerate_protocol(job.meeting_id.clone(), job.template_name.clone());
                    match self.job_repo.enqueue(&proto).await {
                        Ok(()) => info!(job_id = %job.id, next = %proto.id, "enqueued chained protocol job"),
                        Err(e) => error!(job_id = %job.id, "failed to enqueue chained protocol job: {e}"),
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use meeting_core::entities::{Job, JobKind, JobStatus, Meeting};
    use meeting_core::fakes::{
        FakeJobRepo, FakeLlmProvider, FakeMeetingFileStore, FakeMeetingRepo, FakeTemplateLoader,
        FakeTranscriber,
    };
    use meeting_core::ports::{JobRepo, MeetingRepo};
    use std::path::PathBuf;

    fn make_worker(jr: Arc<FakeJobRepo>, mr: Arc<FakeMeetingRepo>) -> Worker {
        Worker::new(
            jr as Arc<dyn JobRepo>,
            mr as Arc<dyn MeetingRepo>,
            FakeTranscriber::new("распознанный текст"),
            FakeMeetingFileStore::new(),
            FakeLlmProvider::new("# Протокол"),
            FakeTemplateLoader::empty(),
            Arc::new(DashMap::new()),
            TRANSCRIBE_KINDS,
            1,
            "transcribe",
        )
    }

    #[tokio::test]
    async fn transcription_with_then_protocol_enqueues_protocol_job() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m = Meeting::new("M".into(), PathBuf::from("/a.wav"));
        mr.save(&m).await.unwrap();

        let mut job = Job::new_reprocess_transcribe(m.id.clone());
        job.then_protocol = true;
        job.template_name = Some("Командная встреча".into());
        jr.enqueue(&job).await.unwrap();

        let worker = make_worker(Arc::clone(&jr), Arc::clone(&mr));
        let claimed = jr.claim_pending(i64::MAX).await.unwrap().unwrap();
        worker.execute(claimed).await;

        // The transcribe job is done...
        let t = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(t.status, JobStatus::Done);
        // ...and a protocol job carrying the template was chained on.
        let proto = jr
            .list_active()
            .await
            .unwrap()
            .into_iter()
            .find(|j| j.kind == JobKind::RegenerateProtocol)
            .expect("chained protocol job should be enqueued");
        assert_eq!(proto.meeting_id, m.id);
        assert_eq!(proto.template_name.as_deref(), Some("Командная встреча"));
    }

    #[tokio::test]
    async fn plain_transcription_does_not_chain_protocol() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m = Meeting::new("M".into(), PathBuf::from("/a.wav"));
        mr.save(&m).await.unwrap();

        let job = Job::new_reprocess_transcribe(m.id.clone()); // then_protocol == false
        jr.enqueue(&job).await.unwrap();

        let worker = make_worker(Arc::clone(&jr), Arc::clone(&mr));
        let claimed = jr.claim_pending(i64::MAX).await.unwrap().unwrap();
        worker.execute(claimed).await;

        assert_eq!(
            jr.find_by_id(&job.id).await.unwrap().unwrap().status,
            JobStatus::Done
        );
        assert!(
            jr.list_active().await.unwrap().is_empty(),
            "no protocol job should be chained for a plain re-transcribe"
        );
    }

    // ── Concurrency / shutdown tests ─────────────────────────────────────────

    use async_trait::async_trait;
    use meeting_core::entities::{Segment, Transcript};
    use meeting_core::ports::Transcriber;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::{Barrier, Notify};

    /// Transcriber that blocks until both concurrent calls reach the barrier.
    struct BarrierTranscriber {
        barrier: Arc<Barrier>,
        active: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Transcriber for BarrierTranscriber {
        async fn transcribe(&self, _audio_path: &Path) -> Result<Transcript, CoreError> {
            let cur = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(cur, Ordering::SeqCst);
            self.barrier.wait().await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(Transcript {
                text: "ok".into(),
                segments: vec![Segment {
                    start_ms: 0,
                    end_ms: 1,
                    text: "ok".into(),
                }],
                language: "ru".into(),
            })
        }
    }

    /// Transcriber that blocks on a Notify until released by the test.
    struct GatedTranscriber {
        gate: Arc<Notify>,
        entered: Arc<Notify>,
    }

    #[async_trait]
    impl Transcriber for GatedTranscriber {
        async fn transcribe(&self, _audio_path: &Path) -> Result<Transcript, CoreError> {
            self.entered.notify_one();
            self.gate.notified().await;
            Ok(Transcript {
                text: "ok".into(),
                segments: vec![Segment {
                    start_ms: 0,
                    end_ms: 1,
                    text: "ok".into(),
                }],
                language: "ru".into(),
            })
        }
    }

    fn worker_with_transcriber(
        jr: Arc<FakeJobRepo>,
        mr: Arc<FakeMeetingRepo>,
        transcriber: Arc<dyn Transcriber>,
        pool_size: usize,
    ) -> Arc<Worker> {
        Arc::new(Worker::new(
            jr as Arc<dyn JobRepo>,
            mr as Arc<dyn MeetingRepo>,
            transcriber,
            FakeMeetingFileStore::new(),
            FakeLlmProvider::new("# Протокол"),
            FakeTemplateLoader::empty(),
            Arc::new(DashMap::new()),
            TRANSCRIBE_KINDS,
            pool_size,
            "transcribe",
        ))
    }

    #[tokio::test]
    async fn pool_size_two_runs_two_transcribes_concurrently() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m1 = Meeting::new("M1".into(), PathBuf::from("/a.wav"));
        let m2 = Meeting::new("M2".into(), PathBuf::from("/b.wav"));
        mr.save(&m1).await.unwrap();
        mr.save(&m2).await.unwrap();

        let j1 = Job::new_transcribe(m1.id.clone());
        let j2 = Job::new_transcribe(m2.id.clone());
        jr.enqueue(&j1).await.unwrap();
        jr.enqueue(&j2).await.unwrap();

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let transcriber = Arc::new(BarrierTranscriber {
            barrier: Arc::new(Barrier::new(2)),
            active: Arc::clone(&active),
            peak: Arc::clone(&peak),
        });

        let worker = worker_with_transcriber(
            Arc::clone(&jr),
            Arc::clone(&mr),
            transcriber as Arc<dyn Transcriber>,
            2,
        );
        let (_tx, rx) = tokio::sync::oneshot::channel::<()>();
        let run = tokio::spawn(worker.run(rx));

        // Wait until both jobs reach Done. If the worker is serial, the
        // barrier deadlocks and this times out.
        let done = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let n_done = jr
                    .list_active()
                    .await
                    .unwrap()
                    .iter()
                    .filter(|j| j.status == JobStatus::Done)
                    .count();
                if n_done == 0 {
                    let j1_done = matches!(
                        jr.find_by_id(&j1.id).await.unwrap().map(|j| j.status),
                        Some(JobStatus::Done)
                    );
                    let j2_done = matches!(
                        jr.find_by_id(&j2.id).await.unwrap().map(|j| j.status),
                        Some(JobStatus::Done)
                    );
                    if j1_done && j2_done {
                        return;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        assert!(done.is_ok(), "both jobs should finish; serial worker deadlocks the barrier");
        assert_eq!(peak.load(Ordering::SeqCst), 2, "two transcribes ran in parallel");

        run.abort();
    }

    #[tokio::test]
    async fn shutdown_blocks_new_claims() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m = Meeting::new("M".into(), PathBuf::from("/a.wav"));
        mr.save(&m).await.unwrap();
        let job = Job::new_transcribe(m.id.clone());
        jr.enqueue(&job).await.unwrap();

        let worker = worker_with_transcriber(
            Arc::clone(&jr),
            Arc::clone(&mr),
            FakeTranscriber::new("ok") as Arc<dyn Transcriber>,
            1,
        );

        // Send shutdown BEFORE spawning so the very first select! wins on shutdown.
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let _ = tx.send(());

        let run = tokio::spawn(worker.run(rx));
        // run should return promptly without claiming the job.
        tokio::time::timeout(Duration::from_secs(2), run)
            .await
            .expect("run should exit on shutdown")
            .unwrap();

        let after = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(after.status, JobStatus::Pending, "job must remain pending");
    }

    #[tokio::test]
    async fn shutdown_drains_inflight_then_exits() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m = Meeting::new("M".into(), PathBuf::from("/a.wav"));
        mr.save(&m).await.unwrap();
        let job = Job::new_transcribe(m.id.clone());
        jr.enqueue(&job).await.unwrap();

        let gate = Arc::new(Notify::new());
        let entered = Arc::new(Notify::new());
        let transcriber = Arc::new(GatedTranscriber {
            gate: Arc::clone(&gate),
            entered: Arc::clone(&entered),
        });
        let worker = worker_with_transcriber(
            Arc::clone(&jr),
            Arc::clone(&mr),
            transcriber as Arc<dyn Transcriber>,
            1,
        );

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let run = tokio::spawn(worker.run(rx));

        // Wait for the job to start.
        tokio::time::timeout(Duration::from_secs(3), entered.notified())
            .await
            .expect("transcribe should start");

        // Request shutdown, then release the gate so the inflight job can finish.
        let _ = tx.send(());
        gate.notify_one();

        tokio::time::timeout(Duration::from_secs(3), run)
            .await
            .expect("run should exit after inflight drains")
            .unwrap();

        let after = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(after.status, JobStatus::Done, "drained job must be marked Done");
    }

    #[tokio::test]
    async fn transcribe_and_protocol_run_in_parallel() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m1 = Meeting::new("M1".into(), PathBuf::from("/a.wav"));
        let m2 = Meeting::new("M2".into(), PathBuf::from("/b.wav"));
        mr.save(&m1).await.unwrap();
        mr.save(&m2).await.unwrap();
        // P-job needs a non-empty transcript on the meeting.
        mr.save_transcript(&m2.id, "transcript body").await.unwrap();

        let t_job = Job::new_transcribe(m1.id.clone());
        let p_job = Job::new_regenerate_protocol(m2.id.clone(), None);
        jr.enqueue(&t_job).await.unwrap();
        jr.enqueue(&p_job).await.unwrap();

        // Transcribe worker: gated transcriber so T(M1) stays running.
        let gate = Arc::new(Notify::new());
        let entered = Arc::new(Notify::new());
        let transcriber = Arc::new(GatedTranscriber {
            gate: Arc::clone(&gate),
            entered: Arc::clone(&entered),
        });
        let t_worker = worker_with_transcriber(
            Arc::clone(&jr),
            Arc::clone(&mr),
            transcriber as Arc<dyn Transcriber>,
            1,
        );

        // Protocol worker: fast LLM, separate pool.
        let p_worker = Arc::new(Worker::new(
            Arc::clone(&jr) as Arc<dyn JobRepo>,
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            FakeTranscriber::new("ok") as Arc<dyn Transcriber>,
            FakeMeetingFileStore::new(),
            FakeLlmProvider::new("# Протокол"),
            FakeTemplateLoader::empty(),
            Arc::new(DashMap::new()),
            PROTOCOL_KINDS,
            2,
            "protocol",
        ));

        let (_t_tx, t_rx) = tokio::sync::oneshot::channel::<()>();
        let (_p_tx, p_rx) = tokio::sync::oneshot::channel::<()>();
        let t_run = tokio::spawn(Arc::clone(&t_worker).run(t_rx));
        let p_run = tokio::spawn(Arc::clone(&p_worker).run(p_rx));

        // T(M1) must enter while T worker holds it.
        tokio::time::timeout(Duration::from_secs(3), entered.notified())
            .await
            .expect("transcribe should start");

        // Now P(M2) must reach Done independently — protocol worker has its own pool.
        let p_done = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(j) = jr.find_by_id(&p_job.id).await.unwrap() {
                    if j.status == JobStatus::Done {
                        return;
                    }
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await;
        assert!(p_done.is_ok(), "protocol job should finish while transcribe is still running");

        // T(M1) is still mid-flight at this point.
        assert_eq!(
            jr.find_by_id(&t_job.id).await.unwrap().unwrap().status,
            JobStatus::Running,
            "transcribe should still be running"
        );

        // Cleanup.
        gate.notify_one();
        t_run.abort();
        p_run.abort();
    }
}
