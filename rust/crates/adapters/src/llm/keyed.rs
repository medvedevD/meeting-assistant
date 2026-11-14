//! The active LLM, with its API key read when minutes are generated rather than
//! when the sidecar starts.
//!
//! After an update of an ad-hoc signed build, macOS asks the user again before
//! the keychain hands a key to the new binary. Reading the key while building
//! the sidecar held it at "Starting core…" until the user found that prompt.
//! Now the prompt, if any, comes when the user asks for minutes.

use std::sync::Arc;

use async_trait::async_trait;
use meeting_core::{
    ports::{LlmCompletion, LlmProvider},
    CoreError,
};

use super::{build_llm, ProviderKind};
use crate::secret_store::KeyringSecretStore;
use crate::settings_store::LlmPrefs;

/// Where the key comes from: called on every generation, and allowed to block
/// (the keychain may be waiting for the user).
pub type KeySource = Arc<dyn Fn(ProviderKind) -> String + Send + Sync>;

pub struct KeyedLlm {
    prefs: LlmPrefs,
    kind: ProviderKind,
    key: KeySource,
}

impl KeyedLlm {
    /// Builds nothing and reads no key until [`LlmProvider::generate`].
    pub fn new(prefs: LlmPrefs, kind: ProviderKind, key: KeySource) -> Self {
        Self { prefs, kind, key }
    }

    /// The active provider of `prefs`, keyed from the user's secret store.
    pub fn active(prefs: &LlmPrefs, secrets: Arc<KeyringSecretStore>) -> Self {
        let source: KeySource = Arc::new(move |kind| secrets.effective_key(kind.as_str()));
        Self::new(prefs.clone(), prefs.active, source)
    }
}

#[async_trait]
impl LlmProvider for KeyedLlm {
    async fn generate(
        &self,
        transcript: &str,
        instructions: Option<&str>,
    ) -> Result<LlmCompletion, CoreError> {
        let source = Arc::clone(&self.key);
        let kind = self.kind;
        let key = tokio::task::spawn_blocking(move || source(kind))
            .await
            .map_err(|e| CoreError::Llm(format!("reading the API key failed: {e}")))?;
        build_llm(&self.prefs.resolve(kind, key))
            .generate(transcript, instructions)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    // Reading the key at startup held the sidecar until the user answered a
    // keychain prompt. Building the LLM must not read it; generating does.
    #[tokio::test]
    async fn reads_the_key_when_generating_not_when_built() {
        let reads = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&reads);
        let source: KeySource = Arc::new(move |kind| {
            assert_eq!(kind, ProviderKind::Ollama);
            counter.fetch_add(1, Ordering::SeqCst);
            String::new()
        });
        let mut prefs = LlmPrefs::default();
        // Nothing listens here, so generate fails fast after reading the key.
        prefs.ollama.base_url = Some("http://127.0.0.1:9/v1".to_string());

        let llm = KeyedLlm::new(prefs, ProviderKind::Ollama, source);
        assert_eq!(reads.load(Ordering::SeqCst), 0, "building read the key");

        let _ = llm.generate("transcript", None).await;
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }
}
