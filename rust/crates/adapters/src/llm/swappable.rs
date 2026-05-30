use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use meeting_core::{ports::LlmProvider, CoreError};

/// An [`LlmProvider`] whose backing implementation can be replaced at runtime
/// (when the user saves new LLM settings) without rebuilding the router or
/// interrupting in-flight requests.
///
/// Held in `AppState` as `Arc<dyn LlmProvider>`; the composition layer keeps a
/// concrete `Arc<SwappableLlm>` to call [`SwappableLlm::set`].
pub struct SwappableLlm {
    current: RwLock<Arc<dyn LlmProvider>>,
}

impl SwappableLlm {
    pub fn new(initial: Arc<dyn LlmProvider>) -> Self {
        Self {
            current: RwLock::new(initial),
        }
    }

    /// Replace the active provider. Requests already in flight keep the
    /// provider they cloned until they finish.
    pub fn set(&self, next: Arc<dyn LlmProvider>) {
        *self.current.write().unwrap() = next;
    }
}

#[async_trait]
impl LlmProvider for SwappableLlm {
    async fn generate(
        &self,
        transcript: &str,
        instructions: Option<&str>,
    ) -> Result<String, CoreError> {
        // Clone the Arc and drop the guard before awaiting (the guard is not Send).
        let provider = { self.current.read().unwrap().clone() };
        provider.generate(transcript, instructions).await
    }
}
