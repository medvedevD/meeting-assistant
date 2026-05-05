use async_trait::async_trait;
use crate::{CoreError, entities::Meeting};

#[async_trait]
pub trait MeetingRepo: Send + Sync {
    async fn save(&self, meeting: &Meeting) -> Result<(), CoreError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Meeting>, CoreError>;
    async fn save_transcript(&self, id: &str, text: &str) -> Result<(), CoreError>;
}
