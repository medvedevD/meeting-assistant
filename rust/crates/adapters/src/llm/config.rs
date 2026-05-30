use serde::{Deserialize, Serialize};

/// Which LLM backend to use. Serialized lowercase to match the wire/JSON form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Anthropic,
    Openai,
    Gemini,
    Mistral,
    Ollama,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Openai => "openai",
            ProviderKind::Gemini => "gemini",
            ProviderKind::Mistral => "mistral",
            ProviderKind::Ollama => "ollama",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "anthropic" | "claude" => Some(ProviderKind::Anthropic),
            "openai" | "chatgpt" | "gpt" => Some(ProviderKind::Openai),
            "gemini" | "google" => Some(ProviderKind::Gemini),
            "mistral" => Some(ProviderKind::Mistral),
            "ollama" => Some(ProviderKind::Ollama),
            _ => None,
        }
    }

    /// Default model identifier for a freshly-configured provider.
    pub fn default_model(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "claude-sonnet-4-6",
            ProviderKind::Openai => "gpt-4o",
            ProviderKind::Gemini => "gemini-2.5-pro",
            ProviderKind::Mistral => "mistral-large-latest",
            ProviderKind::Ollama => "llama3.1:8b",
        }
    }

    /// Default base URL. Cloud providers have fixed endpoints; Ollama is local.
    pub fn default_base_url(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "https://api.anthropic.com",
            ProviderKind::Openai => "https://api.openai.com/v1",
            ProviderKind::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            ProviderKind::Mistral => "https://api.mistral.ai/v1",
            ProviderKind::Ollama => "http://localhost:11434/v1",
        }
    }

    /// Whether this provider authenticates with an API key. Ollama is local and keyless.
    pub fn needs_key(self) -> bool {
        !matches!(self, ProviderKind::Ollama)
    }
}

/// A fully-resolved configuration for building one provider instance.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub kind: ProviderKind,
    pub model: String,
    pub max_tokens: u32,
    pub base_url: String,
    /// Resolved secret (env override or stored). Empty for keyless providers.
    pub api_key: String,
}
