use async_trait::async_trait;
use serde_json::Value;

/// Abstracts persisted settings + secrets for the settings routes. The web
/// layer (this crate) depends only on this trait; the composition layer wires a
/// concrete implementation over the JSON settings store, the OS keyring, and
/// the live adapter handles (LLM/transcriber/templates) for hot-swap.
///
/// The wire shape is `serde_json::Value` so the persisted schema lives in one
/// place (the composition layer) rather than being duplicated as DTOs here.
#[async_trait]
pub trait SettingsService: Send + Sync {
    /// Current settings, **sanitized** — secret values are never included; each
    /// provider carries a `has_key: bool` instead.
    fn snapshot(&self) -> Value;

    /// Persist a full settings object and hot-apply it. Returns the new
    /// sanitized snapshot. `Err` carries a user-facing message (HTTP 400).
    async fn update(&self, body: Value) -> Result<Value, String>;

    /// Store (`Some`) or delete (`None`) a provider's API key.
    async fn set_secret(&self, provider: String, value: Option<String>) -> Result<(), String>;

    /// Cheap credential/connectivity probe for the "Test key" button.
    async fn test_provider(&self, provider: String) -> Result<(), String>;
}
