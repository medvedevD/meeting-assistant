use crate::CoreError;
use async_trait::async_trait;

#[async_trait]
pub trait TemplateLoader: Send + Sync {
    /// Load template text by name. Returns `None` if template doesn't exist.
    async fn load(&self, name: &str) -> Result<Option<String>, CoreError>;

    /// List all available template names.
    async fn list_names(&self) -> Result<Vec<String>, CoreError>;

    /// Create or overwrite the template `name` with `body`.
    ///
    /// `name` is the bare template name (no extension, no path separators); the
    /// caller is responsible for validating it (see
    /// [`crate::usecases::save_template`]).
    async fn save(&self, name: &str, body: &str) -> Result<(), CoreError>;

    /// Delete the template `name`. Returns [`CoreError::NotFound`] if it does
    /// not exist.
    async fn delete(&self, name: &str) -> Result<(), CoreError>;

    /// Rename `old` to `new`. Returns [`CoreError::NotFound`] if `old` does not
    /// exist and [`CoreError::AlreadyExists`] if `new` is already taken.
    async fn rename(&self, old: &str, new: &str) -> Result<(), CoreError>;
}
