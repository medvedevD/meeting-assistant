use std::path::Path;
use std::sync::Arc;
use serde::Serialize;
use crate::{CoreError, ports::{MeetingFileStore, MeetingRepo}};

/// An audio file found on disk that has no corresponding meeting in the DB.
#[derive(Debug, Clone, Serialize)]
pub struct ScanCandidate {
    pub path: String,
    pub name: String,
}

/// Default ceiling on directory recursion for "Из папки" scans.
pub const DEFAULT_MAX_DEPTH: usize = 4;

/// List audio files under `dir` that are not yet tracked as meetings.
///
/// Directory traversal (with depth limit + early-stop) lives in the file-store
/// adapter; this use-case filters the result against the repository.
pub async fn scan_recordings_dir(
    meeting_repo: Arc<dyn MeetingRepo>,
    file_store: Arc<dyn MeetingFileStore>,
    dir: &Path,
    max_depth: usize,
) -> Result<Vec<ScanCandidate>, CoreError> {
    let files = file_store.list_audio_files(dir, max_depth).await?;
    let mut out = Vec::new();
    for path in files {
        if meeting_repo.find_by_audio_path(&path).await?.is_none() {
            out.push(ScanCandidate {
                name: file_name(&path),
                path: path.display().to_string(),
            });
        }
    }
    Ok(out)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::entities::Meeting;
    use crate::fakes::{FakeMeetingFileStore, FakeMeetingRepo};

    #[tokio::test]
    async fn excludes_already_tracked_files() {
        let mr = FakeMeetingRepo::new();
        let tracked = Meeting::new("Tracked".into(), PathBuf::from("/rec/a.wav"));
        mr.save(&tracked).await.unwrap();

        let fs = FakeMeetingFileStore::with_audio_files([
            PathBuf::from("/rec/a.wav"),
            PathBuf::from("/rec/b.mp3"),
        ]);

        let found = scan_recordings_dir(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            fs as Arc<dyn MeetingFileStore>,
            Path::new("/rec"),
            DEFAULT_MAX_DEPTH,
        )
        .await
        .unwrap();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "b.mp3");
        assert_eq!(found[0].path, "/rec/b.mp3");
    }
}
