use crate::entities::JobProgress;
use dashmap::DashMap;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// One entry in the live-jobs table: the in-flight `JobProgress` snapshot and
/// the cancellation token the worker checks at every safe checkpoint. Both
/// fields share an identical lifetime — created when the worker claims a job,
/// dropped from the map when the worker reaches a terminal state. See
/// `plans/done/job-cancellation` (Decision #2).
#[derive(Clone)]
pub struct LiveEntry {
    pub progress: JobProgress,
    pub cancel: CancellationToken,
}

/// A change notification for a single live job. Carries only the job id (a
/// "dirty" signal) plus the kind of change. Subscribers read the *current*
/// snapshot from the table (or the DB on terminal) rather than trusting the
/// payload, so a coalesced or lagged notification can never produce a stale
/// frame. See `plans/done/job-progress-sse` (ADR-001).
#[derive(Clone, Debug)]
pub enum ProgressEvent {
    /// A progress snapshot was written for this job (it is claimed / running).
    Snapshot { job_id: String, progress: JobProgress },
    /// The job left the live table (it reached a terminal state). A subscriber
    /// should read the persisted job from the DB to obtain the final status.
    Terminal { job_id: String },
}

/// Live, in-memory job table keyed by job id, plus a broadcast side-channel so
/// readers (the `GET /jobs/:id/events` SSE handler) can subscribe to changes
/// instead of polling. Shared between the worker (the **sole** writer) and the
/// `GET /jobs/:id`, `GET /active-jobs`, and `DELETE /jobs/:id` handlers
/// (readers / cancel-triggers). Never persisted.
///
/// **Drift guarantee:** the worker must mutate through [`LiveProgress::seed`],
/// [`LiveProgress::publish`], and [`LiveProgress::finish`] so a notification is
/// never forgotten. Readers use the inherited `DashMap` API via [`Deref`]
/// (`get`, `iter`, …) and never need to notify — `Deref` exposes only the
/// read/lookup surface they already rely on.
pub struct LiveProgress {
    jobs: DashMap<String, LiveEntry>,
    tx: broadcast::Sender<ProgressEvent>,
}

impl LiveProgress {
    /// Channel depth. A slow SSE consumer that falls this far behind receives a
    /// `Lagged` error and recovers by reading the current snapshot, so the only
    /// cost of a too-small buffer is an occasional resync, never a lost update.
    const CHANNEL_CAPACITY: usize = 256;

    /// Create an empty table wrapped in an `Arc` (the shared `LiveJobs` alias).
    pub fn new() -> Arc<Self> {
        let (tx, _rx) = broadcast::channel(Self::CHANNEL_CAPACITY);
        Arc::new(Self {
            jobs: DashMap::new(),
            tx,
        })
    }

    /// Subscribe to change notifications. The caller must read the current
    /// snapshot via [`Deref`] (`get`) once *after* subscribing to close the gap
    /// between "read initial state" and "start receiving events".
    pub fn subscribe(&self) -> broadcast::Receiver<ProgressEvent> {
        self.tx.subscribe()
    }

    /// Insert the initial entry for a freshly-claimed job (carrying a brand-new
    /// cancellation token) and notify subscribers. Used by the worker at claim
    /// time, before the per-job task is spawned.
    pub fn seed(&self, job_id: String, entry: LiveEntry) {
        let progress = entry.progress.clone();
        self.jobs.insert(job_id.clone(), entry);
        let _ = self.tx.send(ProgressEvent::Snapshot { job_id, progress });
    }

    /// Upsert a progress snapshot, **preserving** the existing cancellation
    /// token (a fresh one is created only if the job is not yet in the table),
    /// then notify subscribers.
    pub fn publish(&self, job_id: &str, progress: JobProgress) {
        match self.jobs.get_mut(job_id) {
            Some(mut e) => e.progress = progress.clone(),
            None => {
                self.jobs.insert(
                    job_id.to_string(),
                    LiveEntry {
                        progress: progress.clone(),
                        cancel: CancellationToken::new(),
                    },
                );
            }
        }
        let _ = self.tx.send(ProgressEvent::Snapshot {
            job_id: job_id.to_string(),
            progress,
        });
    }

    /// Drop a job's entry on reaching a terminal state and notify subscribers,
    /// so an open SSE stream can read the persisted final status and close.
    pub fn finish(&self, job_id: &str) {
        self.jobs.remove(job_id);
        let _ = self.tx.send(ProgressEvent::Terminal {
            job_id: job_id.to_string(),
        });
    }
}

impl Deref for LiveProgress {
    type Target = DashMap<String, LiveEntry>;
    fn deref(&self) -> &Self::Target {
        &self.jobs
    }
}

/// Shared handle to the live-progress table. `Arc<LiveProgress>` so the worker,
/// the API handlers, and every open SSE stream observe the same state.
pub type LiveJobs = Arc<LiveProgress>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::PipelineStage;

    fn progress(percent: u8) -> JobProgress {
        JobProgress::new(PipelineStage::Transcribing, "Распознавание речи", percent)
    }

    #[tokio::test]
    async fn publish_notifies_and_preserves_cancel_token() {
        let live = LiveProgress::new();
        let mut rx = live.subscribe();

        // Seed establishes a token; publish must keep it across updates.
        let token = CancellationToken::new();
        live.seed(
            "j1".into(),
            LiveEntry {
                progress: progress(0),
                cancel: token.clone(),
            },
        );
        // seed → Snapshot
        match rx.recv().await.unwrap() {
            ProgressEvent::Snapshot { job_id, progress } => {
                assert_eq!(job_id, "j1");
                assert_eq!(progress.percent, 0);
            }
            other => panic!("expected Snapshot, got {other:?}"),
        }

        live.publish("j1", progress(42));
        match rx.recv().await.unwrap() {
            ProgressEvent::Snapshot { job_id, progress } => {
                assert_eq!(job_id, "j1");
                assert_eq!(progress.percent, 42);
            }
            other => panic!("expected Snapshot, got {other:?}"),
        }

        // The cancellation token survived the publish (same entry, mutated in place).
        let entry = live.get("j1").expect("entry present");
        entry.cancel.cancel();
        assert!(token.is_cancelled(), "publish must not replace the cancel token");
    }

    #[tokio::test]
    async fn finish_removes_entry_and_emits_terminal() {
        let live = LiveProgress::new();
        let mut rx = live.subscribe();
        live.publish("j1", progress(10));
        let _ = rx.recv().await.unwrap(); // Snapshot

        live.finish("j1");
        match rx.recv().await.unwrap() {
            ProgressEvent::Terminal { job_id } => assert_eq!(job_id, "j1"),
            other => panic!("expected Terminal, got {other:?}"),
        }
        assert!(live.get("j1").is_none(), "finish must drop the entry");
    }

    #[tokio::test]
    async fn send_without_subscribers_is_harmless() {
        // No subscribers → send returns Err internally, which we swallow.
        let live = LiveProgress::new();
        live.publish("j1", progress(1));
        live.finish("j1");
        assert!(live.get("j1").is_none());
    }
}
