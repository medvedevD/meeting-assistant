pub mod entities;
pub mod ports;
pub mod usecases;

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
}
