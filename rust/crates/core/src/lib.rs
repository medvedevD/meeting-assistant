pub mod entities;
pub mod ports;
pub mod usecases;

mod live;
pub use live::LiveProgress;

#[cfg(any(test, feature = "fakes"))]
pub mod fakes;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("transcription failed: {0}")]
    Transcription(String),
    #[error("audio file not found: {0}")]
    AudioFileNotFound(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("template error: {0}")]
    Template(String),
    #[error("recording error: {0}")]
    Recording(String),
    #[error("authentication failed: {0}")]
    ApiAuth(String),
    #[error("rate limit or quota exceeded: {0}")]
    ApiQuota(String),
    #[error("request timed out: {0}")]
    ApiTimeout(String),
    #[error("audio corrupt: {0}")]
    AudioCorrupt(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("worker terminated: {0}")]
    WorkerKilled(String),
}
