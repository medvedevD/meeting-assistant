use std::path::Path;
use std::sync::Arc;
use crate::{CoreError, entities::Transcript, ports::Transcriber};

pub async fn transcribe_audio_file(
    transcriber: Arc<dyn Transcriber>,
    audio_path: &Path,
) -> Result<Transcript, CoreError> {
    if !audio_path.exists() {
        return Err(CoreError::AudioFileNotFound(
            audio_path.display().to_string(),
        ));
    }
    transcriber.transcribe(audio_path).await
}
