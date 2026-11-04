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

    /// "Test key" check: a one-token `generateContent` against the configured
    /// model, so a model the key can't use fails here rather than only at
    /// protocol generation (listing models succeeds regardless).
    pub async fn probe(&self) -> Result<(), CoreError> {
        let body = GenerateContentRequest {
            contents: vec![Content {
                parts: vec![Part { text: "ping" }],
            }],
            generation_config: GenerationConfig {
                max_output_tokens: 1,
            },
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
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            Err(classify_http("Gemini", status, &text))
        }
    }

    /// Text-generation model ids the key can use (`GET /models`), without the
    /// `models/` prefix. Only `generateContent` models qualify, minus the
    /// speech/image/live variants that support it but can't write a protocol.
    pub async fn list_models(&self) -> Result<Vec<String>, CoreError> {
        let url = format!(
            "{}/models?pageSize=1000&key={}",
            self.base_url, self.api_key
        );
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|e| classify_transport("Gemini", &e))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(classify_http("Gemini", status, &text));
        }
        let parsed: ModelList = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(format!("Gemini: unexpected /models response: {e}")))?;
        Ok(parsed
            .models
            .into_iter()
            .filter(|m| {
                m.supported_generation_methods
                    .iter()
                    .any(|g| g == "generateContent")
            })
            .map(|m| m.name.trim_start_matches("models/").to_string())
            .filter(|id| !NON_TEXT_MARKERS.iter().any(|x| id.contains(x)))
            .collect())
    }
}

/// Id fragments of `generateContent` models whose output isn't text.
const NON_TEXT_MARKERS: &[&str] = &["tts", "image", "live", "native-audio"];

#[derive(Deserialize)]
struct ModelList {
    #[serde(default)]
    models: Vec<ModelEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelEntry {
    name: String,
    #[serde(default)]
    supported_generation_methods: Vec<String>,
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

/// `promptTokenCount` already includes cached content. Thinking models bill
/// `thoughtsTokenCount` as output on top of `candidatesTokenCount`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    prompt_token_count: Option<u32>,
    candidates_token_count: Option<u32>,
    thoughts_token_count: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    usage_metadata: Option<UsageMetadata>,
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn generate(
        &self,
        transcript: &str,
        instructions: Option<&str>,
    ) -> Result<LlmCompletion, CoreError> {
        let mut parts = Vec::new();
        if let Some(instr) = instructions {
            parts.push(Part { text: instr });
        }
        parts.push(Part { text: transcript });

        let body = GenerateContentRequest {
            contents: vec![Content { parts }],
            generation_config: GenerationConfig {
                max_output_tokens: self.max_tokens,
            },
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

        let parsed: GenerateContentResponse = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(e.to_string()))?;
        let text = parsed
            .candidates
            .into_iter()
            .filter_map(|c| c.content)
            .flat_map(|c| c.parts)
            .filter_map(|p| p.text)
            .collect::<Vec<_>>()
            .join("");
        let usage = parsed.usage_metadata.and_then(|u| {
            token_usage(
                &[u.prompt_token_count],
                &[u.candidates_token_count, u.thoughts_token_count],
            )
        });
        Ok(LlmCompletion { text, usage })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meeting_core::entities::TokenUsage;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, method, path, path_regex};
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
        let out = provider
            .generate("transcript", Some("be brief"))
            .await
            .unwrap();
        assert_eq!(out.text, "# Protocol");
        assert_eq!(out.usage, None);
    }

    #[tokio::test]
    async fn counts_thinking_tokens_as_output() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"/models/.*:generateContent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"parts": [{"text": "# Protocol"}]}}],
                "usageMetadata": {
                    "promptTokenCount": 5200,
                    "candidatesTokenCount": 800,
                    "thoughtsTokenCount": 1500,
                    "totalTokenCount": 7500
                }
            })))
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("k", "gemini-2.5-pro", 4096, server.uri());
        let out = provider.generate("transcript", None).await.unwrap();
        assert_eq!(
            out.usage,
            Some(TokenUsage {
                input_tokens: 5200,
                output_tokens: 2300,
            })
        );
    }

    #[tokio::test]
    async fn list_models_keeps_text_generation_models_only() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "models": [
                    {"name": "models/gemini-2.5-pro",
                     "supportedGenerationMethods": ["generateContent", "countTokens"]},
                    {"name": "models/text-embedding-004",
                     "supportedGenerationMethods": ["embedContent"]},
                    {"name": "models/gemini-2.5-flash-preview-tts",
                     "supportedGenerationMethods": ["generateContent"]},
                    {"name": "models/gemini-2.5-flash",
                     "supportedGenerationMethods": ["generateContent"]}
                ]
            })))
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("k", "gemini-2.5-pro", 4096, server.uri());
        assert_eq!(
            provider.list_models().await.unwrap(),
            vec!["gemini-2.5-pro", "gemini-2.5-flash"]
        );
    }

    #[tokio::test]
    async fn classifies_403_as_model_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"/models/.*:generateContent"))
            .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("k", "gemini-2.5-pro", 4096, server.uri());
        let err = provider.generate("t", None).await.unwrap_err();
        assert!(matches!(err, CoreError::LlmModelUnavailable(_)));
    }

    #[tokio::test]
    async fn probe_requests_one_token_from_the_configured_model() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"/models/gemini-2\.5-flash:generateContent"))
            .and(body_partial_json(
                json!({"generationConfig": {"maxOutputTokens": 1}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"candidates": []})))
            .expect(1)
            .mount(&server)
            .await;

        let provider = GeminiProvider::new("k", "gemini-2.5-flash", 4096, server.uri());
        provider.probe().await.unwrap();
    }
}
