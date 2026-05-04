use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::Result;
use meeting_core::ports::Transcriber;
use meeting_adapters::WhisperTranscriber;

pub struct Container {
    pub transcriber: Arc<dyn Transcriber>,
}

impl Container {
    pub fn new_desktop(model_path: &Path) -> Result<Self> {
        let transcriber = WhisperTranscriber::new(model_path)?;
        Ok(Self { transcriber: Arc::new(transcriber) })
    }
}

/// Returns `~/.local/share/meeting-assistant/models/ggml-base.bin`,
/// respecting XDG_DATA_HOME if set.
pub fn default_model_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("$HOME is not set");
            PathBuf::from(home).join(".local/share")
        });
    base.join("meeting-assistant/models/ggml-medium.bin")
}
