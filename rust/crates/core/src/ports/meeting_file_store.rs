use std::path::{Path, PathBuf};
use async_trait::async_trait;
use crate::CoreError;

#[async_trait]
pub trait MeetingFileStore: Send + Sync {
    async fn write_transcript(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError>;
    async fn write_protocol(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError>;
}
