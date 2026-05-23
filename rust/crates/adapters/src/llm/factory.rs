use std::sync::Arc;

use meeting_core::{ports::LlmProvider, CoreError};

use super::anthropic::AnthropicProvider;
use super::config::{LlmConfig, ProviderKind};
use super::gemini::GeminiProvider;
use super::openai_compat::OpenAiCompatProvider;

/// Build a concrete [`LlmProvider`] for the given resolved config.
pub fn build_llm(cfg: &LlmConfig) -> Arc<dyn LlmProvider> {
    match cfg.kind {
        ProviderKind::Anthropic => Arc::new(
            AnthropicProvider::with_base_url(cfg.api_key.clone(), cfg.base_url.clone())
                .with_model(cfg.model.clone())
                .with_max_tokens(cfg.max_tokens),
        ),
        ProviderKind::Gemini => Arc::new(GeminiProvider::new(
            cfg.api_key.clone(),
            cfg.model.clone(),
            cfg.max_tokens,
            cfg.base_url.clone(),
        )),
        ProviderKind::Openai | ProviderKind::Mistral | ProviderKind::Ollama => {
            Arc::new(OpenAiCompatProvider::new(
                cfg.kind.as_str(),
                cfg.api_key.clone(),
                cfg.model.clone(),
                cfg.max_tokens,
                cfg.base_url.clone(),
            ))
        }
    }
}

/// Cheap credential/connectivity probe for the "Test key" button.
pub async fn probe_llm(cfg: &LlmConfig) -> Result<(), CoreError> {
    match cfg.kind {
        ProviderKind::Anthropic => {
            AnthropicProvider::with_base_url(cfg.api_key.clone(), cfg.base_url.clone())
                .with_model(cfg.model.clone())
                .probe()
                .await
        }
        ProviderKind::Gemini => {
            GeminiProvider::new(
                cfg.api_key.clone(),
                cfg.model.clone(),
                cfg.max_tokens,
                cfg.base_url.clone(),
            )
            .probe()
            .await
        }
        ProviderKind::Openai | ProviderKind::Mistral | ProviderKind::Ollama => {
            OpenAiCompatProvider::new(
                cfg.kind.as_str(),
                cfg.api_key.clone(),
                cfg.model.clone(),
                cfg.max_tokens,
                cfg.base_url.clone(),
            )
            .probe()
            .await
        }
    }
}
