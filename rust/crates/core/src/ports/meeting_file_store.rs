use crate::CoreError;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[async_trait]
pub trait MeetingFileStore: Send + Sync {
    async fn write_transcript(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError>;
    async fn write_protocol(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError>;
    /// Copy an external audio file into `dir`, returning the new path. Used by
    /// the import flow when the source is copied (rather than registered in place).
    async fn import_audio(&self, dir: &Path, source: &Path) -> Result<PathBuf, CoreError>;
    /// List audio files (.wav/.mp3/.m4a) under `dir`, descending at most
    /// `max_depth` levels, with an internal cap to keep scans cheap.
    async fn list_audio_files(
        &self,
        dir: &Path,
        max_depth: usize,
    ) -> Result<Vec<PathBuf>, CoreError>;
    /// Best-effort removal of a single file (succeeds if already gone).
    async fn remove_file(&self, path: &Path) -> Result<(), CoreError>;
    /// Best-effort recursive removal of a directory (succeeds if already gone).
    async fn remove_dir_all(&self, dir: &Path) -> Result<(), CoreError>;
}
