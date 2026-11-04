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
            Arc::new(openai_compat(cfg))
        }
    }
}

/// The OpenAI-compatible adapter for `cfg`, with OpenAI's own token-cap field
/// (its reasoning and GPT-5 models reject `max_tokens`).
fn openai_compat(cfg: &LlmConfig) -> OpenAiCompatProvider {
    let provider = OpenAiCompatProvider::new(
        cfg.kind.as_str(),
        cfg.api_key.clone(),
        cfg.model.clone(),
        cfg.max_tokens,
        cfg.base_url.clone(),
    );
    match cfg.kind {
        ProviderKind::Openai => provider.with_max_completion_tokens(),
        _ => provider,
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
            openai_compat(cfg).probe().await
        }
    }
}

/// Models the configured key can use, for the settings model picker: each
/// provider's own listing, narrowed to models that can write a protocol.
/// A listing proves the key *sees* a model, not that the subscription tier
/// allows it — [`probe_llm`] is the check for that.
pub async fn list_llm_models(cfg: &LlmConfig) -> Result<Vec<String>, CoreError> {
    match cfg.kind {
        ProviderKind::Anthropic => {
            AnthropicProvider::with_base_url(cfg.api_key.clone(), cfg.base_url.clone())
                .list_models()
                .await
        }
        ProviderKind::Gemini => {
            GeminiProvider::new(
                cfg.api_key.clone(),
                cfg.model.clone(),
                cfg.max_tokens,
                cfg.base_url.clone(),
            )
            .list_models()
            .await
        }
        ProviderKind::Openai => Ok(openai_compat(cfg)
            .list_models()
            .await?
            .into_iter()
            .filter(|id| is_openai_chat_model(id))
            .collect()),
        ProviderKind::Mistral | ProviderKind::Ollama => openai_compat(cfg).list_models().await,
    }
}

/// Id fragments of OpenAI models that aren't Chat Completions text models:
/// speech, images, search, legacy completions, and Responses-API-only models
/// (`codex`, `deep-research`, `computer-use`, the `-pro` tier).
const OPENAI_NON_CHAT_MARKERS: &[&str] = &[
    "audio",
    "realtime",
    "transcribe",
    "tts",
    "image",
    "search",
    "instruct",
    "codex",
    "deep-research",
    "computer-use",
    "-pro",
];

/// Whether an OpenAI model id can write a protocol over Chat Completions.
/// OpenAI's `/models` lists everything the key reaches (embeddings, whisper,
/// dall-e, moderation…) with no capability flags, so this goes by naming:
/// the `gpt-`/`chatgpt-`/`o<digit>` families, minus non-chat variants.
fn is_openai_chat_model(id: &str) -> bool {
    let mut chars = id.chars();
    let o_series = chars.next() == Some('o') && chars.next().is_some_and(|c| c.is_ascii_digit());
    let family = id.starts_with("gpt-") || id.starts_with("chatgpt-") || o_series;
    family && !OPENAI_NON_CHAT_MARKERS.iter().any(|m| id.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_listing_keeps_chat_models_only() {
        let listed = [
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-pro",
            "gpt-5-codex",
            "gpt-4o",
            "gpt-4o-2024-08-06",
            "gpt-4o-mini-tts",
            "gpt-4o-transcribe",
            "gpt-4o-realtime-preview",
            "gpt-4o-search-preview",
            "gpt-audio",
            "gpt-image-1",
            "gpt-3.5-turbo-instruct",
            "chatgpt-4o-latest",
            "o3",
            "o4-mini",
            "o3-deep-research",
            "o1-pro",
            "omni-moderation-latest",
            "text-embedding-3-large",
            "whisper-1",
            "dall-e-3",
            "tts-1",
            "computer-use-preview",
        ];
        let kept: Vec<&str> = listed
            .into_iter()
            .filter(|id| is_openai_chat_model(id))
            .collect();
        assert_eq!(
            kept,
            vec![
                "gpt-5",
                "gpt-5-mini",
                "gpt-4o",
                "gpt-4o-2024-08-06",
                "chatgpt-4o-latest",
                "o3",
                "o4-mini",
            ]
        );
    }
}
