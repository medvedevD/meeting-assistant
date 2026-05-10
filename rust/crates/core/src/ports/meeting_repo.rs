use std::path::Path;
use async_trait::async_trait;
use crate::{CoreError, entities::Meeting};

#[async_trait]
pub trait MeetingRepo: Send + Sync {
    async fn save(&self, meeting: &Meeting) -> Result<(), CoreError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Meeting>, CoreError>;
    async fn find_by_audio_path(&self, path: &Path) -> Result<Option<Meeting>, CoreError>;
    async fn save_transcript(&self, id: &str, text: &str) -> Result<(), CoreError>;
    async fn save_protocol(&self, id: &str, text: &str) -> Result<(), CoreError>;
    async fn save_transcript_file(&self, id: &str, text: &str, path: &Path) -> Result<(), CoreError>;
    async fn save_protocol_file(&self, id: &str, text: &str, path: &Path) -> Result<(), CoreError>;
    async fn update_name(&self, id: &str, name: &str) -> Result<(), CoreError>;
    async fn list_all(&self) -> Result<Vec<Meeting>, CoreError>;
}
