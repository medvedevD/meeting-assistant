use std::path::Path;
use async_trait::async_trait;
use crate::{CoreError, entities::Transcript};

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, audio_path: &Path) -> Result<Transcript, CoreError>;
}
