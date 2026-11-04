use serde::{Deserialize, Serialize};

/// Tokens an LLM provider reported for one generation. Input counts every
/// prompt token the provider billed, cached ones included; output counts the
/// generated tokens, reasoning/thinking included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl TokenUsage {
    pub fn total(&self) -> u32 {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}
