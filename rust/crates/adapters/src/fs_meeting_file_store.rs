use std::path::{Path, PathBuf};
use async_trait::async_trait;
use meeting_core::{CoreError, ports::MeetingFileStore};

pub struct FsMeetingFileStore;

#[async_trait]
impl MeetingFileStore for FsMeetingFileStore {
    async fn write_transcript(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        tokio::fs::create_dir_all(dir).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let path = dir.join("transcript.md");
        tokio::fs::write(&path, text).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(path)
    }

    async fn write_protocol(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        tokio::fs::create_dir_all(dir).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let path = dir.join("protocol.md");
        tokio::fs::write(&path, text).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(path)
    }
}
