use async_trait::async_trait;
use meeting_core::{ports::MeetingFileStore, CoreError};
use std::path::{Path, PathBuf};

/// True for the Windows errors raised when a file is still held open by another
/// process: ERROR_SHARING_VIOLATION (32) and ERROR_LOCK_VIOLATION (33). On other
/// platforms an open file can be unlinked, so this is always false.
#[cfg(windows)]
fn is_sharing_violation(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(32) | Some(33))
}
#[cfg(not(windows))]
fn is_sharing_violation(_e: &std::io::Error) -> bool {
    false
}

/// Run a filesystem removal, retrying briefly on a Windows sharing/lock
/// violation. Deleting a recording can race the GUI's media player releasing its
/// file handle (QtMultimedia tears playback down on a worker thread), which
/// surfaces as ERROR_SHARING_VIOLATION; the handle is gone a moment later, so a
/// short bounded retry turns a spurious failure into success. `NotFound` is
/// treated as success (already gone); any other error fails fast.
async fn remove_with_retry<F, Fut>(mut op: F) -> std::io::Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::io::Result<()>>,
{
    const ATTEMPTS: usize = 10;
    const DELAY_MS: u64 = 100;
    for attempt in 0..ATTEMPTS {
        match op().await {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) if is_sharing_violation(&e) && attempt + 1 < ATTEMPTS => {
                tokio::time::sleep(std::time::Duration::from_millis(DELAY_MS)).await;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub struct FsMeetingFileStore;

#[async_trait]
impl MeetingFileStore for FsMeetingFileStore {
    async fn write_transcript(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let path = dir.join("transcript.md");
        tokio::fs::write(&path, text)
            .await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(path)
    }

    async fn write_protocol(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let path = dir.join("protocol.md");
        tokio::fs::write(&path, text)
            .await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(path)
    }

    async fn import_audio(&self, dir: &Path, source: &Path) -> Result<PathBuf, CoreError> {
        let name = source.file_name().ok_or_else(|| {
            CoreError::Validation(format!("source has no file name: {}", source.display()))
        })?;
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let dest = dir.join(name);
        tokio::fs::copy(source, &dest).await.map_err(|e| {
            CoreError::Storage(format!(
                "copy {} → {}: {e}",
                source.display(),
                dest.display()
            ))
        })?;
        Ok(dest)
    }

    async fn list_audio_files(
        &self,
        dir: &Path,
        max_depth: usize,
    ) -> Result<Vec<PathBuf>, CoreError> {
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
                    if depth < max_depth {
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
        remove_with_retry(|| tokio::fs::remove_file(path))
            .await
            .map_err(|e| CoreError::Storage(e.to_string()))
    }

    async fn remove_dir_all(&self, dir: &Path) -> Result<(), CoreError> {
        remove_with_retry(|| tokio::fs::remove_dir_all(dir))
            .await
            .map_err(|e| CoreError::Storage(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io;

    // Regression (Windows): a recording deletion that races the media player's
    // handle release fails the first attempt(s) with ERROR_SHARING_VIOLATION (os
    // error 32), then succeeds once the handle is gone. remove_with_retry must
    // retry those and return Ok. The retry only engages on Windows, where
    // is_sharing_violation recognises code 32/33.
    #[cfg(windows)]
    #[tokio::test(start_paused = true)]
    async fn retries_sharing_violation_then_succeeds() {
        let calls = Cell::new(0u32);
        let res = remove_with_retry(|| {
            let n = calls.get();
            calls.set(n + 1);
            async move {
                if n < 2 {
                    Err(io::Error::from_raw_os_error(32)) // ERROR_SHARING_VIOLATION
                } else {
                    Ok(())
                }
            }
        })
        .await;
        assert!(res.is_ok(), "should succeed once the handle is released");
        assert_eq!(calls.get(), 3, "two violations then a success");
    }

    #[cfg(windows)]
    #[tokio::test(start_paused = true)]
    async fn gives_up_on_persistent_sharing_violation() {
        let res = remove_with_retry(|| async { Err(io::Error::from_raw_os_error(32)) }).await;
        assert!(res.is_err(), "a permanently held file still fails");
    }

    // A non-retryable error (e.g. permission denied) fails immediately without
    // burning the retry budget — only file-in-use violations are retried.
    #[tokio::test(start_paused = true)]
    async fn does_not_retry_other_errors() {
        let calls = Cell::new(0u32);
        let res = remove_with_retry(|| {
            calls.set(calls.get() + 1);
            async { Err(io::Error::new(io::ErrorKind::PermissionDenied, "nope")) }
        })
        .await;
        assert!(res.is_err());
        assert_eq!(
            calls.get(),
            1,
            "must not retry a non-sharing-violation error"
        );
    }

    // NotFound means the target is already gone — treated as success.
    #[tokio::test(start_paused = true)]
    async fn not_found_is_success() {
        let res =
            remove_with_retry(|| async { Err(io::Error::from(io::ErrorKind::NotFound)) }).await;
        assert!(res.is_ok());
    }
}
