mod anthropic;
pub use anthropic::AnthropicProvider;

mod config;
pub use config::{LlmConfig, ProviderKind};

mod errors;

/// What the "Test key" probe asks the configured model. It goes out with the
/// same parameters as a protocol request, token cap included: a reasoning
/// model spends that budget on thinking first, so a smaller cap can be
/// rejected even though generation with the configured one succeeds. A short
/// fixed answer keeps the real cost to a few tokens.
const PROBE_PROMPT: &str = "Reply with OK.";

mod usage;

mod openai_compat;
pub use openai_compat::OpenAiCompatProvider;

mod gemini;
pub use gemini::GeminiProvider;

mod factory;
pub use factory::{build_llm, list_llm_models, probe_llm};

mod swappable;
pub use swappable::SwappableLlm;

mod keyed;
pub use keyed::{KeySource, KeyedLlm};
