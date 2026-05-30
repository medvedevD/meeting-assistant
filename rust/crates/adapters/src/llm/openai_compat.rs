use std::time::Duration;

use async_trait::async_trait;
use meeting_core::{ports::LlmProvider, CoreError};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::errors::{classify_http, classify_transport};

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
    client: Client,
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
            client,
        }
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.api_key.is_empty() {
            rb
        } else {
            rb.bearer_auth(&self.api_key)
        }
    }

    /// Cheap credential/connectivity check: `GET {base_url}/models`.
    pub async fn probe(&self) -> Result<(), CoreError> {
        let url = format!("{}/models", self.base_url);
        let resp = self
            .auth(self.client.get(&url))
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
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    max_tokens: u32,
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

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn generate(
        &self,
        transcript: &str,
        instructions: Option<&str>,
    ) -> Result<String, CoreError> {
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

        let body = ChatRequest {
            model: &self.model,
            max_tokens: self.max_tokens,
            messages,
        };
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
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_chat_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "# Protocol"}}]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("OpenAI", "k", "gpt-4o", 4096, server.uri());
        let out = provider
            .generate("transcript", Some("be brief"))
            .await
            .unwrap();
        assert_eq!(out, "# Protocol");
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
    async fn probe_hits_models_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
            .mount(&server)
            .await;

        let provider = OpenAiCompatProvider::new("OpenAI", "k", "gpt-4o", 4096, server.uri());
        assert!(provider.probe().await.is_ok());
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
        assert_eq!(out, "# Мистраль");
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
        assert_eq!(out, "# Оллама");
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
