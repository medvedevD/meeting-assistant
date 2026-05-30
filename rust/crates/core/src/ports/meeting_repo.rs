use crate::{entities::Meeting, CoreError};
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait MeetingRepo: Send + Sync {
    async fn save(&self, meeting: &Meeting) -> Result<(), CoreError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Meeting>, CoreError>;
    async fn find_by_audio_path(&self, path: &Path) -> Result<Option<Meeting>, CoreError>;
    async fn save_transcript(&self, id: &str, text: &str) -> Result<(), CoreError>;
    async fn save_protocol(&self, id: &str, text: &str) -> Result<(), CoreError>;
    async fn save_transcript_file(
        &self,
        id: &str,
        text: &str,
        path: &Path,
    ) -> Result<(), CoreError>;
    async fn save_protocol_file(&self, id: &str, text: &str, path: &Path) -> Result<(), CoreError>;
    async fn update_name(&self, id: &str, name: &str) -> Result<(), CoreError>;
    async fn list_all(&self) -> Result<Vec<Meeting>, CoreError>;
    /// Forget the meeting's audio (its file was deleted) while keeping the
    /// transcript and protocol. Clears `audio_path`.
    async fn delete_audio_only(&self, id: &str) -> Result<(), CoreError>;
    /// Clear transcript text + path (used before re-transcribing).
    async fn clear_transcript(&self, id: &str) -> Result<(), CoreError>;
    /// Clear protocol text + path (used before regenerating).
    async fn clear_protocol(&self, id: &str) -> Result<(), CoreError>;
    /// Permanently remove the meeting row (and any of its jobs).
    async fn delete(&self, id: &str) -> Result<(), CoreError>;
}
