use std::time::Duration;
use async_trait::async_trait;
use meeting_core::{CoreError, ports::LlmProvider};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::errors::{classify_http, classify_transport};

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    max_tokens: u32,
    base_url: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, "https://api.anthropic.com")
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()
            .expect("failed to build HTTP client");
        Self {
            api_key: api_key.into(),
            model: "claude-sonnet-4-6".to_string(),
            max_tokens: 8192,
            base_url: base_url.into(),
            client,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Cheap credential check: a 1-token completion. Surfaces auth/quota errors
    /// without running a full protocol generation.
    pub async fn probe(&self) -> Result<(), CoreError> {
        let body = RequestBody {
            model: &self.model,
            max_tokens: 1,
            messages: vec![Message {
                role: "user",
                content: vec![ContentBlock { kind: "text", text: "hi", cache_control: None }],
            }],
        };
        let url = format!("{}/v1/messages", self.base_url);
        let resp = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| classify_transport("Anthropic", &e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            Err(classify_http("Anthropic", status, &text))
        }
    }
}

// ── Anthropic API types ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct ContentBlock<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

#[derive(Serialize)]
struct CacheControl {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'static str,
    content: Vec<ContentBlock<'a>>,
}

#[derive(Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<Message<'a>>,
}

#[derive(Deserialize)]
struct ResponseContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ResponseBody {
    content: Vec<ResponseContent>,
    stop_reason: Option<String>,
}

// ── LlmProvider impl ─────────────────────────────────────────────────────────

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn generate(&self, transcript: &str, instructions: Option<&str>) -> Result<String, CoreError> {
        let mut content = vec![ContentBlock {
            kind: "text",
            text: transcript,
            cache_control: Some(CacheControl { kind: "ephemeral" }),
        }];
        if let Some(instr) = instructions {
            content.push(ContentBlock { kind: "text", text: instr, cache_control: None });
        }

        let body = RequestBody {
            model: &self.model,
            max_tokens: self.max_tokens,
            messages: vec![Message { role: "user", content }],
        };

        let url = format!("{}/v1/messages", self.base_url);
        let resp = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| classify_transport("Anthropic", &e))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(classify_http("Anthropic", status, &text));
        }

        let parsed: ResponseBody = resp.json().await.map_err(|e| CoreError::Llm(e.to_string()))?;

        if parsed.stop_reason.as_deref() == Some("max_tokens") {
            tracing::warn!("Anthropic response truncated: max_tokens reached");
        }

        let text = parsed
            .content
            .into_iter()
            .filter(|c| c.kind == "text")
            .filter_map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");

        Ok(text)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header, body_json};
    use serde_json::json;

    async fn mock_server_with_response(_response_body: serde_json::Value) -> (MockServer, AnthropicProvider) {
        let server = MockServer::start().await;
        let provider = AnthropicProvider::with_base_url("test-api-key", server.uri());
        (server, provider)
    }

    #[tokio::test]
    async fn returns_generated_text_without_instructions() {
        let (server, provider) = mock_server_with_response(json!({})).await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "test-api-key"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [{"type": "text", "text": "# Protocol\n\nDone."}],
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let result = provider.generate("transcript text", None).await.unwrap();
        assert_eq!(result, "# Protocol\n\nDone.");
    }

    #[tokio::test]
    async fn sends_instructions_as_second_content_block() {
        let (server, provider) = mock_server_with_response(json!({})).await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(body_json(json!({
                "model": "claude-sonnet-4-6",
                "max_tokens": 8192,
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "transcript", "cache_control": {"type": "ephemeral"}},
                        {"type": "text", "text": "make it short"}
                    ]
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [{"type": "text", "text": "Short."}],
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let result = provider.generate("transcript", Some("make it short")).await.unwrap();
        assert_eq!(result, "Short.");
    }

    #[tokio::test]
    async fn returns_llm_error_on_api_failure() {
        let (server, provider) = mock_server_with_response(json!({})).await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let err = provider.generate("transcript", None).await.unwrap_err();
        assert!(matches!(err, CoreError::Network(_)));
    }

    #[tokio::test]
    async fn returns_llm_error_on_401() {
        let (server, provider) = mock_server_with_response(json!({})).await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {"type": "authentication_error", "message": "invalid api key"}
            })))
            .mount(&server)
            .await;

        let err = provider.generate("t", None).await.unwrap_err();
        assert!(matches!(err, CoreError::ApiAuth(_)));
    }

    #[tokio::test]
    async fn respects_custom_model() {
        let (server, provider) = mock_server_with_response(json!({})).await;
        let provider = provider.with_model("claude-opus-4-7");

        Mock::given(method("POST"))
            .and(body_json(json!({
                "model": "claude-opus-4-7",
                "max_tokens": 8192,
                "messages": [{
                    "role": "user",
                    "content": [{"type": "text", "text": "hi", "cache_control": {"type": "ephemeral"}}]
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [{"type": "text", "text": "ok"}],
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let result = provider.generate("hi", None).await.unwrap();
        assert_eq!(result, "ok");
    }
}
