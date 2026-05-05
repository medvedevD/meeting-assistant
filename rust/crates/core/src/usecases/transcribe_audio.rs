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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeTranscriber;

    #[tokio::test]
    async fn returns_error_for_missing_file() {
        let t = FakeTranscriber::new("irrelevant");
        let result = transcribe_audio_file(t, Path::new("/nonexistent/file.wav")).await;
        assert!(matches!(result, Err(CoreError::AudioFileNotFound(_))));
    }

    #[tokio::test]
    async fn returns_transcript_for_existing_file() {
        let mut tmp = tempfile().unwrap();
        tmp.write_all(b"dummy").unwrap();
        let path = tmp.path().to_path_buf();

        let t = FakeTranscriber::new("привет мир");
        let transcript = transcribe_audio_file(t, &path).await.unwrap();
        assert_eq!(transcript.text, "привет мир");
        assert_eq!(transcript.language, "ru");
    }

    fn tempfile() -> std::io::Result<tempfile_impl::NamedTempFile> {
        tempfile_impl::NamedTempFile::new()
    }

    mod tempfile_impl {
        use std::{fs, path::PathBuf};

        pub struct NamedTempFile {
            pub path: PathBuf,
            file: fs::File,
        }

        impl NamedTempFile {
            pub fn new() -> std::io::Result<Self> {
                let path = std::env::temp_dir().join(format!(
                    "meeting-test-{}.wav",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .subsec_nanos()
                ));
                let file = fs::File::create(&path)?;
                Ok(Self { path, file })
            }

            pub fn path(&self) -> &std::path::Path { &self.path }
            pub fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
                use std::io::Write;
                self.file.write_all(buf)
            }
        }

        impl Drop for NamedTempFile {
            fn drop(&mut self) { let _ = fs::remove_file(&self.path); }
        }
    }
}
