use crate::{
    entities::{PipelineStage, Transcript},
    CoreError,
};
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Sink the transcriber calls to report fine-grained progress: a pipeline
/// stage plus a 0–100 percent within it. Implementations may call it many
/// times; callers must be cheap and non-blocking.
pub type ProgressSink = Arc<dyn Fn(PipelineStage, u8) + Send + Sync>;

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, audio_path: &Path) -> Result<Transcript, CoreError>;

    /// Transcribe while reporting progress through `on_progress`. The default
    /// ignores progress and delegates to [`Transcriber::transcribe`] — fakes
    /// and simple adapters need not override it.
    async fn transcribe_with_progress(
        &self,
        audio_path: &Path,
        _on_progress: ProgressSink,
    ) -> Result<Transcript, CoreError> {
        self.transcribe(audio_path).await
    }

    /// Transcribe with progress reporting **and** cooperative cancellation.
    /// Implementations that can interrupt their inference (e.g. the Whisper
    /// adapter via `set_abort_callback_safe`) wire the token in; the default
    /// ignores it and delegates to [`Transcriber::transcribe_with_progress`].
    /// Used by the worker for jobs that may be cancelled via
    /// `DELETE /api/v1/jobs/:id`. See `plans/active/job-cancellation`.
    async fn transcribe_cancelable(
        &self,
        audio_path: &Path,
        on_progress: ProgressSink,
        _cancel: CancellationToken,
    ) -> Result<Transcript, CoreError> {
        self.transcribe_with_progress(audio_path, on_progress).await
    }
}
