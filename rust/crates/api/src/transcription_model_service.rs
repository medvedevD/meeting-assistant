use async_trait::async_trait;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionModelsView {
    pub models: Vec<TranscriptionModelView>,
    pub selected_model_id: Option<String>,
    pub active_source: String,
    pub models_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionModelView {
    pub id: String,
    pub display_name: String,
    pub approximate_size: String,
    pub description: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub filename: String,
    pub download_url: String,
    pub checksum: String,
    pub installed: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallStarted {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallationView {
    pub job_id: String,
    pub model_id: String,
    pub status: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ModelServiceError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
}

impl ModelServiceError {
    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest(message)
            | Self::NotFound(message)
            | Self::Conflict(message)
            | Self::Internal(message) => message,
        }
    }
}

#[async_trait]
pub trait TranscriptionModelService: Send + Sync {
    async fn list(&self) -> Result<TranscriptionModelsView, ModelServiceError>;
    async fn start_install(&self, model_id: String) -> Result<InstallStarted, ModelServiceError>;
    async fn installation(&self, job_id: String) -> Result<InstallationView, ModelServiceError>;
    async fn delete_model(&self, model_id: String) -> Result<(), ModelServiceError>;
}
