use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;
use crate::{CoreError, entities::{PipelineStage, Transcript}};

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
}
