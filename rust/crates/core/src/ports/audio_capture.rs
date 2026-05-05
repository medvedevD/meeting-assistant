use std::path::Path;
use async_trait::async_trait;
use crate::CoreError;

#[async_trait]
pub trait AudioCapture: Send + Sync {
    /// Begin capturing mic audio, writing PCM to `output_path`.
    async fn start_session(&self, session_id: &str, output_path: &Path) -> Result<(), CoreError>;

    /// Stop the session, finalize the output file, and wait for the writer to flush.
    async fn stop_session(&self, session_id: &str) -> Result<(), CoreError>;

    fn is_active(&self, session_id: &str) -> bool;
}
