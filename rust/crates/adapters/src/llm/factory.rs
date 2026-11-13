use std::sync::Arc;

use meeting_core::{entities::ErrorClass, ports::LlmProvider, CoreError};

use super::anthropic::AnthropicProvider;
use super::config::{LlmConfig, ProviderKind};
use super::gemini::GeminiProvider;
use super::openai_compat::OpenAiCompatProvider;

/// Build a concrete [`LlmProvider`] for the given resolved config.
pub fn build_llm(cfg: &LlmConfig) -> Arc<dyn LlmProvider> {
    match cfg.kind {
        ProviderKind::Anthropic => Arc::new(anthropic(cfg)),
        ProviderKind::Gemini => Arc::new(gemini(cfg)),
        ProviderKind::Openai | ProviderKind::Mistral | ProviderKind::Ollama => {
            Arc::new(openai_compat(cfg))
        }
    }
}

fn anthropic(cfg: &LlmConfig) -> AnthropicProvider {
    AnthropicProvider::with_base_url(cfg.api_key.clone(), cfg.base_url.clone())
        .with_model(cfg.model.clone())
        .with_max_tokens(cfg.max_tokens)
}

fn gemini(cfg: &LlmConfig) -> GeminiProvider {
    GeminiProvider::new(
        cfg.api_key.clone(),
        cfg.model.clone(),
        cfg.max_tokens,
        cfg.base_url.clone(),
    )
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

/// Credential/connectivity probe for the "Test key" button: a one-line request
/// to the configured model, built exactly as [`build_llm`] builds the provider,
/// so it is accepted wherever a protocol request is. A failure is logged, since
/// the settings screen shows only its class.
pub async fn probe_llm(cfg: &LlmConfig) -> Result<(), CoreError> {
    let result = match cfg.kind {
        ProviderKind::Anthropic => anthropic(cfg).probe().await,
        ProviderKind::Gemini => gemini(cfg).probe().await,
        ProviderKind::Openai | ProviderKind::Mistral | ProviderKind::Ollama => {
            openai_compat(cfg).probe().await
        }
    };
    if let Err(e) = &result {
        tracing::warn!(
            provider = cfg.kind.as_str(),
            model = %cfg.model,
            error_class = ErrorClass::from_core_error(e).as_str(),
            "LLM key check failed: {}",
            without_key(&e.to_string(), &cfg.api_key)
        );
    }
    result
}

/// `text` with every copy of `key` masked, in case a provider's message
/// quotes the key it was sent.
fn without_key(text: &str, key: &str) -> String {
    if key.is_empty() {
        text.to_string()
    } else {
        text.replace(key, "<api key>")
    }
}

/// Models the configured key can use, for the settings model picker: each
/// provider's own listing, narrowed to models that can write a protocol.
/// A listing proves the key *sees* a model, not that the subscription tier
/// allows it — [`probe_llm`] is the check for that.
pub async fn list_llm_models(cfg: &LlmConfig) -> Result<Vec<String>, CoreError> {
    match cfg.kind {
        ProviderKind::Anthropic => anthropic(cfg).list_models().await,
        ProviderKind::Gemini => gemini(cfg).list_models().await,
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
    use crate::test_log::LogBuf;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Mount a provider that, like a reasoning model, accepts the configured
    /// cap and rejects any smaller one with a 400.
    async fn needs_the_configured_cap(server: &MockServer, kind: ProviderKind, cap: u32) {
        let (endpoint, body) = match kind {
            ProviderKind::Anthropic => (path("/v1/messages"), json!({"max_tokens": cap})),
            ProviderKind::Gemini => (
                path("/models/m:generateContent"),
                json!({"generationConfig": {"maxOutputTokens": cap}}),
            ),
            ProviderKind::Openai => (
                path("/chat/completions"),
                json!({"max_completion_tokens": cap}),
            ),
            ProviderKind::Mistral | ProviderKind::Ollama => {
                (path("/chat/completions"), json!({"max_tokens": cap}))
            }
        };
        Mock::given(method("POST"))
            .and(endpoint)
            .and(body_partial_json(body))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [{"type": "text", "text": "OK"}],
                "candidates": [{"content": {"parts": [{"text": "OK"}]}}],
                "choices": [{"message": {"role": "assistant", "content": "OK"}}]
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path_regex(".*"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": {"message": "max tokens too small to finish reasoning"}
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn a_failed_probe_is_logged_with_provider_model_and_class() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": {"message": "Unsupported value: 'temperature'"}
            })))
            .mount(&server)
            .await;
        let cfg = LlmConfig {
            kind: ProviderKind::Openai,
            model: "gpt-6-astra".into(),
            max_tokens: 25000,
            base_url: server.uri(),
            api_key: "sk-test-secret".into(),
        };
        let log = LogBuf::default();
        let _guard = log.capture("info");

        assert!(probe_llm(&cfg).await.is_err());

        let line = log.line_with("LLM key check failed");
        assert!(line.contains("WARN"), "{line}");
        assert!(line.contains("provider=\"openai\""), "{line}");
        assert!(line.contains("model=gpt-6-astra"), "{line}");
        assert!(line.contains("error_class=\"llm_rejected\""), "{line}");
        assert!(line.contains("Unsupported value: 'temperature'"), "{line}");
        assert!(!line.contains("sk-test-secret"), "{line}");
    }

    #[tokio::test]
    async fn a_logged_probe_failure_never_carries_the_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {"message": "Incorrect API key provided: sk-test-secret."}
            })))
            .mount(&server)
            .await;
        let cfg = LlmConfig {
            kind: ProviderKind::Openai,
            model: "gpt-6-astra".into(),
            max_tokens: 25000,
            base_url: server.uri(),
            api_key: "sk-test-secret".into(),
        };
        let log = LogBuf::default();
        let _guard = log.capture("info");

        assert!(probe_llm(&cfg).await.is_err());

        let line = log.line_with("LLM key check failed");
        assert!(line.contains("error_class=\"api_auth\""), "{line}");
        assert!(
            line.contains("Incorrect API key provided: <api key>."),
            "{line}"
        );
        assert!(!line.contains("sk-test-secret"), "{line}");
    }

    #[tokio::test]
    async fn probe_passes_wherever_generation_with_the_configured_cap_does() {
        for kind in [
            ProviderKind::Anthropic,
            ProviderKind::Openai,
            ProviderKind::Gemini,
            ProviderKind::Mistral,
            ProviderKind::Ollama,
        ] {
            let server = MockServer::start().await;
            needs_the_configured_cap(&server, kind, 25000).await;
            let cfg = LlmConfig {
                kind,
                model: "m".into(),
                max_tokens: 25000,
                base_url: server.uri(),
                api_key: "k".into(),
            };

            build_llm(&cfg)
                .generate("transcript", None)
                .await
                .unwrap_or_else(|e| panic!("{kind:?} generate: {e:?}"));
            probe_llm(&cfg)
                .await
                .unwrap_or_else(|e| panic!("{kind:?} probe: {e:?}"));
        }
    }

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
