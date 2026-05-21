use std::time::Duration;

use async_trait::async_trait;
use meeting_core::{ports::LlmProvider, CoreError};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::errors::{classify_http, classify_transport};

/// Google Gemini provider (Generative Language API). Distinct wire format from
/// the OpenAI-compatible providers: model is in the URL path, key is a query
/// param, and content uses `contents[].parts[].text`.
pub struct GeminiProvider {
    api_key: String,
    model: String,
    max_tokens: u32,
    base_url: String,
    client: Client,
}

impl GeminiProvider {
    pub fn new(
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
            api_key: api_key.into(),
            model: model.into(),
            max_tokens,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Cheap credential check: list models for the key.
    pub async fn probe(&self) -> Result<(), CoreError> {
        let url = format!("{}/models?key={}", self.base_url, self.api_key);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| classify_transport("Gemini", &e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            Err(classify_http("Gemini", status, &text))
        }
    }
}

#[derive(Serialize)]
struct Part<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct Content<'a> {
    parts: Vec<Part<'a>>,
}

#[derive(Serialize)]
struct GenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Serialize)]
struct GenerateContentRequest<'a> {
    contents: Vec<Content<'a>>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Deserialize)]
struct RespPart {
    text: Option<String>,
}

#[derive(Deserialize)]
struct RespContent {
    #[serde(default)]
    parts: Vec<RespPart>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<RespContent>,
}

#[derive(Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn generate(&self, transcript: &str, instructions: Option<&str>) -> Result<String, CoreError> {
        let mut parts = Vec::new();
        if let Some(instr) = instructions {
            parts.push(Part { text: instr });
        }
        parts.push(Part { text: transcript });

        let body = GenerateContentRequest {
            contents: vec![Content { parts }],
            generation_config: GenerationConfig { max_output_tokens: self.max_tokens },
        };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, self.model, self.api_key
        );
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| classify_transport("Gemini", &e))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(classify_http("Gemini", status, &text));
        }

        let parsed: GenerateContentResponse =
            resp.json().await.map_err(|e| CoreError::Llm(e.to_string()))?;
        let text = parsed
            .candidates
            .into_iter()
            .filter_map(|c| c.content)
            .flat_map(|c| c.parts)
            .filter_map(|p| p.text)
            .collect::<Vec<_>>()
            .join("");
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_generate_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"/models/.*:generateContent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "# Protocol"}]}}]
            })))
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("k", "gemini-2.5-pro", 4096, server.uri());
        let out = provider.generate("transcript", Some("be brief")).await.unwrap();
        assert_eq!(out, "# Protocol");
    }

    #[tokio::test]
    async fn classifies_403_as_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"/models/.*:generateContent"))
            .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("k", "gemini-2.5-pro", 4096, server.uri());
        let err = provider.generate("t", None).await.unwrap_err();
        assert!(matches!(err, CoreError::ApiAuth(_)));
    }
}
