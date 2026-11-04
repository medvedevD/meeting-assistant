use std::time::Duration;

use async_trait::async_trait;
use meeting_core::{
    ports::{LlmCompletion, LlmProvider},
    CoreError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::errors::{classify_http, classify_transport};
use super::usage::token_usage;

/// Provider for any OpenAI-compatible Chat Completions endpoint:
/// OpenAI/ChatGPT, Mistral, and Ollama (local) all speak this wire format.
/// They differ only by `base_url`, default model, and whether a key is sent.
pub struct OpenAiCompatProvider {
    /// Human-readable provider name, used in error messages.
    label: String,
    api_key: String,
    model: String,
    max_tokens: u32,
    /// Base URL up to (but excluding) `/chat/completions`, e.g.
    /// `https://api.openai.com/v1`.
    base_url: String,
    token_limit: TokenLimitParam,
    client: Client,
}

/// Which request field carries the output-token cap.
#[derive(Clone, Copy)]
enum TokenLimitParam {
    /// `max_tokens` — Mistral, Ollama and other OpenAI-compatible servers.
    MaxTokens,
    /// `max_completion_tokens` — OpenAI, whose reasoning and GPT-5 models
    /// reject the legacy `max_tokens` (all its chat models accept this one).
    MaxCompletionTokens,
}

impl OpenAiCompatProvider {
    pub fn new(
        label: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        max_tokens: u32,
        base_url: impl Into<String>,
    ) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()
            .expect("failed to build HTTP client");
        Self {
            label: label.into(),
            api_key: api_key.into(),
            model: model.into(),
            max_tokens,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token_limit: TokenLimitParam::MaxTokens,
            client,
        }
    }

    /// Send the output cap as `max_completion_tokens` (OpenAI's API).
    pub fn with_max_completion_tokens(mut self) -> Self {
        self.token_limit = TokenLimitParam::MaxCompletionTokens;
        self
    }

    fn chat_request<'a>(&'a self, limit: u32, messages: Vec<ChatMessage<'a>>) -> ChatRequest<'a> {
        let (max_tokens, max_completion_tokens) = match self.token_limit {
            TokenLimitParam::MaxTokens => (Some(limit), None),
            TokenLimitParam::MaxCompletionTokens => (None, Some(limit)),
        };
        ChatRequest {
            model: &self.model,
            max_tokens,
            max_completion_tokens,
            messages,
        }
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.api_key.is_empty() {
            rb
        } else {
            rb.bearer_auth(&self.api_key)
        }
    }

    /// "Test key" check: the request `generate` sends, cut to one output token,
    /// against the configured model. Listing `/models` is not enough — it
    /// succeeds for a valid key even when the model is outside the
    /// subscription tier, which then only fails at protocol generation.
    pub async fn probe(&self) -> Result<(), CoreError> {
        let body = self.chat_request(
            1,
            vec![ChatMessage {
                role: "user",
                content: "ping",
            }],
        );
        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(|e| classify_transport(&self.label, &e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            Err(classify_http(&self.label, status, &text))
        }
    }

    /// Model ids the key can see (`GET {base_url}/models`), newest first.
    /// Mistral flags chat-capable models via `capabilities.completion_chat`;
    /// entries declaring it false (embeddings, OCR, moderation) are dropped.
    /// OpenAI's list carries no capabilities — `list_llm_models` filters it.
    pub async fn list_models(&self) -> Result<Vec<String>, CoreError> {
        let url = format!("{}/models", self.base_url);
        let resp = self
            .auth(self.client.get(&url))
            .timeout(LIST_MODELS_TIMEOUT)
            .send()
            .await
            .map_err(|e| classify_transport(&self.label, &e))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(classify_http(&self.label, status, &text));
        }
        let parsed: ModelList = resp.json().await.map_err(|e| {
            CoreError::Llm(format!("{}: unexpected /models response: {e}", self.label))
        })?;

        let mut models: Vec<ModelEntry> = parsed
            .data
            .into_iter()
            .filter(|m| {
                m.capabilities
                    .as_ref()
                    .and_then(|c| c.completion_chat)
                    .unwrap_or(true)
            })
            .collect();
        models.sort_by(|a, b| {
            b.created
                .unwrap_or(0)
                .cmp(&a.created.unwrap_or(0))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(models.into_iter().map(|m| m.id).collect())
    }
}

/// A model listing is a small metadata call — don't let it hang the settings
/// screen for the 300 s a protocol generation is allowed.
const LIST_MODELS_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Deserialize)]
struct ModelList {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    created: Option<i64>,
    #[serde(default)]
    capabilities: Option<ModelCapabilities>,
}

#[derive(Deserialize)]
struct ModelCapabilities {
    #[serde(default)]
    completion_chat: Option<bool>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

/// `completion_tokens` already includes any reasoning tokens.
#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn generate(
        &self,
        transcript: &str,
        instructions: Option<&str>,
    ) -> Result<LlmCompletion, CoreError> {
        let mut messages = Vec::new();
        if let Some(instr) = instructions {
            messages.push(ChatMessage {
                role: "system",
                content: instr,
            });
        }
        messages.push(ChatMessage {
            role: "user",
            content: transcript,
        });

        let body = self.chat_request(self.max_tokens, messages);
        let url = format!("{}/chat/completions", self.base_url);

        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(|e| classify_transport(&self.label, &e))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(classify_http(&self.label, status, &text));
        }

        let parsed: ChatResponse = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(e.to_string()))?;
        let text = parsed
            .choices
            .into_iter()
            .filter_map(|c| c.message.content)
            .collect::<Vec<_>>()
            .join("");
        let usage = parsed
            .usage
            .and_then(|u| token_usage(&[u.prompt_tokens], &[u.completion_tokens]));
        Ok(LlmCompletion { text, usage })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meeting_core::entities::TokenUsage;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_chat_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "# Protocol"}}],
                "usage": {"prompt_tokens": 4100, "completion_tokens": 620, "total_tokens": 4720}
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("OpenAI", "k", "gpt-4o", 4096, server.uri());
        let out = provider
            .generate("transcript", Some("be brief"))
            .await
            .unwrap();
        assert_eq!(out.text, "# Protocol");
        assert_eq!(
            out.usage,
            Some(TokenUsage {
                input_tokens: 4100,
                output_tokens: 620,
            })
        );
    }

    #[tokio::test]
    async fn a_null_usage_block_does_not_fail_the_generation() {
        // Local OpenAI-compatible servers may send usage with null counters; the
        // paid/finished response must still come through.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "# Protocol"}}],
                "usage": {"prompt_tokens": null, "completion_tokens": null}
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("Ollama", "", "llama3.1:8b", 4096, server.uri());
        let out = provider.generate("transcript", None).await.unwrap();
        assert_eq!(out.text, "# Protocol");
        assert_eq!(out.usage, None);
    }

    #[tokio::test]
    async fn classifies_429_as_quota() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429).set_body_string("slow down"))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("OpenAI", "k", "gpt-4o", 4096, server.uri());
        let err = provider.generate("t", None).await.unwrap_err();
        assert!(matches!(err, CoreError::ApiQuota(_)));
    }

    #[tokio::test]
    async fn probe_requests_one_token_from_the_configured_model() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(
                json!({"model": "gpt-4o-mini", "max_tokens": 1}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "p"}}]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("OpenAI", "k", "gpt-4o-mini", 4096, server.uri());
        assert!(provider.probe().await.is_ok());
    }

    #[tokio::test]
    async fn probe_fails_when_the_model_is_outside_the_subscription_tier() {
        // A valid key lists `/models` fine; only a request to the model itself
        // reveals the tier restriction that later failed protocol generation.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "object": "error",
                "message": "This model is not available in your subscription tier",
                "type": "tier_not_allowed",
                "param": null,
                "code": "1910",
                "raw_status_code": 403
            })))
            .mount(&server)
            .await;

        let provider =
            OpenAiCompatProvider::new("mistral", "k", "mistral-large-latest", 4096, server.uri());
        let err = provider.probe().await.unwrap_err();
        assert!(
            matches!(err, CoreError::LlmModelUnavailable(_)),
            "got {err:?}"
        );
        assert!(err
            .to_string()
            .contains("This model is not available in your subscription tier"));
    }

    #[tokio::test]
    async fn mistral_parses_chat_completion() {
        // Mistral speaks the same Chat Completions wire format; it differs only
        // by label/base_url/model, so the same path must serve it.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("authorization", "Bearer mk"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "# Мистраль"}}]
            })))
            .mount(&server)
            .await;

        let provider =
            OpenAiCompatProvider::new("Mistral", "mk", "mistral-large-latest", 4096, server.uri());
        let out = provider.generate("transcript", None).await.unwrap();
        assert_eq!(out.text, "# Мистраль");
    }

    #[tokio::test]
    async fn ollama_is_keyless_and_still_generates() {
        // Ollama is local and keyless: an empty key must not break the request
        // (the provider omits the Authorization header — see `auth`).
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "# Оллама"}}]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("Ollama", "", "llama3.1:8b", 4096, server.uri());
        let out = provider.generate("transcript", None).await.unwrap();
        assert_eq!(out.text, "# Оллама");
        assert_eq!(out.usage, None);
    }

    #[tokio::test]
    async fn openai_sends_max_completion_tokens_instead_of_max_tokens() {
        // o-series / GPT-5 reject `max_tokens`; the OpenAI wiring must send the
        // field every OpenAI chat model accepts — for both generate and probe.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(json!({"max_completion_tokens": 4096})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "# P"}}]
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(json!({"max_completion_tokens": 1})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": ""}}]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("openai", "k", "o3", 4096, server.uri())
            .with_max_completion_tokens();
        provider.generate("t", None).await.unwrap();
        provider.probe().await.unwrap();

        let requests = server.received_requests().await.unwrap();
        for r in &requests {
            let body: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
            assert!(body.get("max_tokens").is_none(), "sent max_tokens: {body}");
        }
    }

    #[tokio::test]
    async fn list_models_drops_non_chat_entries_and_sorts_newest_first() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("authorization", "Bearer mk"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list",
                "data": [
                    {"id": "mistral-small-latest", "created": 100,
                     "capabilities": {"completion_chat": true}},
                    {"id": "mistral-embed", "created": 300,
                     "capabilities": {"completion_chat": false}},
                    {"id": "mistral-medium-latest", "created": 200,
                     "capabilities": {"completion_chat": true}},
                    {"id": "no-capabilities-field", "created": 50}
                ]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("mistral", "mk", "m", 4096, server.uri());
        assert_eq!(
            provider.list_models().await.unwrap(),
            vec![
                "mistral-medium-latest",
                "mistral-small-latest",
                "no-capabilities-field"
            ]
        );
    }

    #[tokio::test]
    async fn list_models_classifies_a_bad_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {"message": "Incorrect API key provided"}
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("openai", "bad", "gpt-4o", 4096, server.uri());
        let err = provider.list_models().await.unwrap_err();
        assert!(matches!(err, CoreError::ApiAuth(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn classifies_401_as_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("OpenAI", "k", "gpt-4o", 4096, server.uri());
        let err = provider.generate("t", None).await.unwrap_err();
        assert!(matches!(err, CoreError::ApiAuth(_)));
    }
}
