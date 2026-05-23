use crate::CoreError;
use async_trait::async_trait;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate protocol markdown from transcript and optional instructions.
    async fn generate(
        &self,
        transcript: &str,
        instructions: Option<&str>,
    ) -> Result<String, CoreError>;
}
