use async_trait::async_trait;
use axum::http::StatusCode;

/// A template plus its body, as returned to the UI for the list+preview view.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TemplateDto {
    pub name: String,
    pub body: String,
}

/// Failure modes the template routes distinguish on the wire. Keeping this in
/// the web layer (rather than leaking `CoreError`) lets the composition layer
/// map its concrete errors onto exactly the HTTP statuses the client expects.
#[derive(Debug)]
pub enum TemplateError {
    /// Name failed validation (e.g. path traversal) → 400.
    Validation(String),
    /// Template does not exist → 404.
    NotFound(String),
    /// Rename target name already taken → 409.
    Conflict(String),
    /// I/O or other unexpected failure → 500.
    Internal(String),
}

impl TemplateError {
    pub fn status(&self) -> StatusCode {
        match self {
            TemplateError::Validation(_) => StatusCode::BAD_REQUEST,
            TemplateError::NotFound(_) => StatusCode::NOT_FOUND,
            TemplateError::Conflict(_) => StatusCode::CONFLICT,
            TemplateError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(self) -> String {
        match self {
            TemplateError::Validation(m)
            | TemplateError::NotFound(m)
            | TemplateError::Conflict(m)
            | TemplateError::Internal(m) => m,
        }
    }
}

/// Abstracts template CRUD for the `/api/v1/templates` routes. Like
/// [`crate::SettingsService`], this lives off [`crate::AppState`] so the routes
/// carry their own state; the composition layer wires a concrete implementation
/// over the [`meeting_core::ports::TemplateLoader`] and the settings store
/// (needed to clear a dangling `default_template` on delete — plan decision #7).
#[async_trait]
pub trait TemplateService: Send + Sync {
    /// Every template with its body.
    async fn list(&self) -> Result<Vec<TemplateDto>, TemplateError>;

    /// One template's body.
    async fn get(&self, name: &str) -> Result<String, TemplateError>;

    /// Create or overwrite a template.
    async fn save(&self, name: &str, body: &str) -> Result<(), TemplateError>;

    /// Delete a template. Returns an optional user-facing warning — e.g. the
    /// deleted template was the configured `default_template`, which the server
    /// has now cleared.
    async fn delete(&self, name: &str) -> Result<Option<String>, TemplateError>;

    /// Rename `old` to `new`.
    async fn rename(&self, old: &str, new: &str) -> Result<(), TemplateError>;
}
