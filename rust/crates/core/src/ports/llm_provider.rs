use crate::{entities::TokenUsage, CoreError};
use async_trait::async_trait;

/// One LLM response: the generated text plus the tokens it cost, when the
/// provider reports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCompletion {
    pub text: String,
    pub usage: Option<TokenUsage>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate protocol markdown from transcript and optional instructions.
    async fn generate(
        &self,
        transcript: &str,
        instructions: Option<&str>,
    ) -> Result<LlmCompletion, CoreError>;
}
