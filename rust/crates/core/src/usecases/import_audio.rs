use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::{
    CoreError,
    entities::{Job, Meeting},
    ports::{JobRepo, MeetingFileStore, MeetingRepo},
};

/// Outcome of an import: the created meeting and, if requested, the queued
/// transcription job.
pub struct ImportResult {
    pub meeting: Meeting,
    pub job: Option<Job>,
}

fn derive_name(source: &Path) -> String {
    source
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Импортированная встреча".to_string())
}

/// Register an external audio file as a new meeting.
///
/// - `copy = true` (drag&drop / FileDialog import): the source is copied into
///   `recordings_dir/<id>/` and the meeting points at the copy; the original
///   stays untouched (decision #9).
/// - `copy = false` ("Из папки" registration): the file is registered in place
///   and the meeting points at the original path.
///
/// Dedup by `audio_path` is the caller's responsibility (the route returns 409);
/// this use-case assumes the path is new.
pub async fn import_audio(
    meeting_repo: Arc<dyn MeetingRepo>,
    job_repo: Arc<dyn JobRepo>,
    file_store: Arc<dyn MeetingFileStore>,
    recordings_dir: &Path,
    source: PathBuf,
    copy: bool,
    enqueue_transcribe: bool,
) -> Result<ImportResult, CoreError> {
    let name = derive_name(&source);
    // Meeting::new mints the id we use for the per-meeting directory.
    let mut meeting = Meeting::new(name, source.clone());

    if copy {
        let dir = recordings_dir.join(&meeting.id);
        let dest = file_store.import_audio(&dir, &source).await?;
        meeting.meeting_dir = dir;
        meeting.audio_path = dest;
    } else {
        meeting.meeting_dir = source
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| recordings_dir.to_path_buf());
        meeting.audio_path = source;
    }

    meeting_repo.save(&meeting).await?;

    let job = if enqueue_transcribe {
        let job = Job::new_transcribe(meeting.id.clone());
        job_repo.enqueue(&job).await?;
        Some(job)
    } else {
        None
    };

    Ok(ImportResult { meeting, job })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeJobRepo, FakeMeetingFileStore, FakeMeetingRepo};

    #[tokio::test]
    async fn copy_import_places_file_under_recordings_dir_and_enqueues() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let fs = FakeMeetingFileStore::new();

        let res = import_audio(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&jr) as Arc<dyn JobRepo>,
            Arc::clone(&fs) as Arc<dyn MeetingFileStore>,
            Path::new("/recordings"),
            PathBuf::from("/downloads/standup.wav"),
            true,
            true,
        )
        .await
        .unwrap();

        assert_eq!(res.meeting.name, "standup");
        assert!(res.meeting.audio_path.starts_with("/recordings"));
        assert!(res.job.is_some());
        // original source must remain referenced for the copy
        let imported = fs.imported.lock().unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].1, PathBuf::from("/downloads/standup.wav"));
    }

    #[tokio::test]
    async fn in_place_import_keeps_source_path_and_skips_job() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let fs = FakeMeetingFileStore::new();
        let src = PathBuf::from("/recordings/old/meeting.m4a");

        let res = import_audio(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&jr) as Arc<dyn JobRepo>,
            Arc::clone(&fs) as Arc<dyn MeetingFileStore>,
            Path::new("/recordings"),
            src.clone(),
            false,
            false,
        )
        .await
        .unwrap();

        assert_eq!(res.meeting.audio_path, src);
        assert!(res.job.is_none());
        assert!(fs.imported.lock().unwrap().is_empty());
    }
}
