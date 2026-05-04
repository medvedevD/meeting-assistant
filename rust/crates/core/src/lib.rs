pub mod entities;
pub mod ports;
pub mod usecases;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("transcription failed: {0}")]
    Transcription(String),
    #[error("audio file not found: {0}")]
    AudioFileNotFound(String),
}
