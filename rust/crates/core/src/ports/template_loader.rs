use async_trait::async_trait;
use crate::CoreError;

#[async_trait]
pub trait TemplateLoader: Send + Sync {
    /// Load template text by name. Returns `None` if template doesn't exist.
    async fn load(&self, name: &str) -> Result<Option<String>, CoreError>;

    /// List all available template names.
    async fn list_names(&self) -> Result<Vec<String>, CoreError>;
}
