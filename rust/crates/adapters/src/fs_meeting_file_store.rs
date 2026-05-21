use std::path::{Path, PathBuf};
use async_trait::async_trait;
use meeting_core::{CoreError, ports::MeetingFileStore};

pub struct FsMeetingFileStore;

#[async_trait]
impl MeetingFileStore for FsMeetingFileStore {
    async fn write_transcript(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        tokio::fs::create_dir_all(dir).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let path = dir.join("transcript.md");
        tokio::fs::write(&path, text).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(path)
    }

    async fn write_protocol(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        tokio::fs::create_dir_all(dir).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let path = dir.join("protocol.md");
        tokio::fs::write(&path, text).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(path)
    }

    async fn import_audio(&self, dir: &Path, source: &Path) -> Result<PathBuf, CoreError> {
        let name = source
            .file_name()
            .ok_or_else(|| CoreError::Validation(format!("source has no file name: {}", source.display())))?;
        tokio::fs::create_dir_all(dir).await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let dest = dir.join(name);
        tokio::fs::copy(source, &dest).await
            .map_err(|e| CoreError::Storage(format!("copy {} → {}: {e}", source.display(), dest.display())))?;
        Ok(dest)
    }

    async fn list_audio_files(&self, dir: &Path, max_depth: usize) -> Result<Vec<PathBuf>, CoreError> {
        // Bounded breadth-first walk; early-stop once we have plenty of
        // candidates so a huge tree never stalls the scan.
        const HARD_CAP: usize = 500;
        const AUDIO_EXTS: &[&str] = &["wav", "mp3", "m4a"];

        let mut out = Vec::new();
        // (path, depth) frontier.
        let mut stack = vec![(dir.to_path_buf(), 0usize)];

        while let Some((current, depth)) = stack.pop() {
            let mut entries = match tokio::fs::read_dir(&current).await {
                Ok(e) => e,
                Err(_) => continue, // unreadable dir: skip, don't fail the whole scan
            };
            loop {
                let entry = match entries.next_entry().await {
                    Ok(Some(e)) => e,
                    Ok(None) => break,
                    Err(_) => break,
                };
                let path = entry.path();
                let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    if depth + 1 <= max_depth {
                        stack.push((path, depth + 1));
                    }
                } else if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
                    .unwrap_or(false)
                {
                    out.push(path);
                    if out.len() >= HARD_CAP {
                        return Ok(out);
                    }
                }
            }
        }
        Ok(out)
    }

    async fn remove_file(&self, path: &Path) -> Result<(), CoreError> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CoreError::Storage(e.to_string())),
        }
    }

    async fn remove_dir_all(&self, dir: &Path) -> Result<(), CoreError> {
        match tokio::fs::remove_dir_all(dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CoreError::Storage(e.to_string())),
        }
    }
}
