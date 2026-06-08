use crate::{
    ports::{MeetingFileStore, MeetingRepo},
    CoreError,
};
use std::path::Path;
use std::sync::Arc;

/// True when `meeting_dir` is a per-meeting directory the app created itself,
/// so a full delete can safely wipe it. Covers two layouts:
///   * `recordings_dir/<uuid>`            — produced by copy-import.
///   * `recordings_dir/<slug>_<uuid8>`    — produced by start_recording.
/// In-place imports may live anywhere (including a shared folder under
/// `recordings_dir`), so they fail this check and only their own files are
/// removed.
fn is_owned_meeting_dir(meeting_dir: &Path, recordings_dir: &Path, meeting_id: &str) -> bool {
    if meeting_dir.parent() != Some(recordings_dir) {
        return false;
    }
    let Some(name) = meeting_dir.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if name == meeting_id {
        return true;
    }
    match meeting_id.get(..8) {
        Some(prefix) if !prefix.is_empty() => name.ends_with(&format!("_{prefix}")),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMode {
    /// Delete the audio file, keep transcript + protocol (meeting stays listed).
    AudioOnly,
    /// Remove the meeting entirely, including its files and job rows.
    Full,
}

/// Delete a meeting's audio or the whole meeting.
///
/// `recordings_dir` is used to recognise meetings whose directory we own
/// (`recordings_dir/<id>`) so a `Full` delete can safely remove that directory.
/// In-place imported meetings live in a shared/foreign directory, so we only
/// remove their individual files, never the directory.
pub async fn delete_meeting(
    meeting_repo: Arc<dyn MeetingRepo>,
    file_store: Arc<dyn MeetingFileStore>,
    recordings_dir: &Path,
    meeting_id: &str,
    mode: DeleteMode,
) -> Result<(), CoreError> {
    let meeting = meeting_repo
        .find_by_id(meeting_id)
        .await?
        .ok_or_else(|| CoreError::NotFound(format!("meeting '{meeting_id}'")))?;

    match mode {
        DeleteMode::AudioOnly => {
            if !meeting.audio_path.as_os_str().is_empty() {
                file_store.remove_file(&meeting.audio_path).await?;
            }
            meeting_repo.delete_audio_only(meeting_id).await?;
        }
        DeleteMode::Full => {
            if is_owned_meeting_dir(&meeting.meeting_dir, recordings_dir, &meeting.id) {
                file_store.remove_dir_all(&meeting.meeting_dir).await?;
            } else {
                // Foreign/shared dir (in-place import): remove only our files.
                if !meeting.audio_path.as_os_str().is_empty() {
                    file_store.remove_file(&meeting.audio_path).await?;
                }
                if let Some(p) = &meeting.transcript_path {
                    file_store.remove_file(p).await?;
                }
                if let Some(p) = &meeting.protocol_path {
                    file_store.remove_file(p).await?;
                }
            }
            meeting_repo.delete(meeting_id).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Meeting;
    use crate::fakes::{FakeMeetingFileStore, FakeMeetingRepo};
    use std::path::PathBuf;

    #[tokio::test]
    async fn audio_only_keeps_meeting_and_clears_audio_path() {
        let mr = FakeMeetingRepo::new();
        let fs = FakeMeetingFileStore::new();
        let m = Meeting::new("M".into(), PathBuf::from("/rec/x/audio.wav"));
        mr.save(&m).await.unwrap();
        mr.save_transcript(&m.id, "kept").await.unwrap();

        delete_meeting(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&fs) as Arc<dyn MeetingFileStore>,
            Path::new("/rec"),
            &m.id,
            DeleteMode::AudioOnly,
        )
        .await
        .unwrap();

        let after = mr.find_by_id(&m.id).await.unwrap().unwrap();
        assert!(after.audio_path.as_os_str().is_empty());
        assert_eq!(after.transcript_text.as_deref(), Some("kept"));
        assert!(fs
            .removed_files
            .lock()
            .unwrap()
            .contains(&PathBuf::from("/rec/x/audio.wav")));
    }

    #[tokio::test]
    async fn full_delete_removes_owned_dir_and_row() {
        let mr = FakeMeetingRepo::new();
        let fs = FakeMeetingFileStore::new();
        let mut m = Meeting::new("M".into(), PathBuf::from("/rec/audio.wav"));
        // Make the dir the one we own: recordings_dir/<id>.
        m.meeting_dir = PathBuf::from("/rec").join(&m.id);
        m.audio_path = m.meeting_dir.join("recording.wav");
        mr.save(&m).await.unwrap();

        delete_meeting(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&fs) as Arc<dyn MeetingFileStore>,
            Path::new("/rec"),
            &m.id,
            DeleteMode::Full,
        )
        .await
        .unwrap();

        assert!(mr.find_by_id(&m.id).await.unwrap().is_none());
        assert!(fs.removed_dirs.lock().unwrap().contains(&m.meeting_dir));
    }

    #[tokio::test]
    async fn full_delete_removes_slug_dir_from_start_recording() {
        let mr = FakeMeetingRepo::new();
        let fs = FakeMeetingFileStore::new();
        let mut m = Meeting::new("M".into(), PathBuf::from("/rec/audio.wav"));
        // Mimic start_recording: dir = recordings_dir/<slug>_<uuid8>.
        let slug = format!("2026-05-22_09-12_{}", &m.id[..8]);
        m.meeting_dir = PathBuf::from("/rec").join(&slug);
        m.audio_path = m.meeting_dir.join("recording.wav");
        mr.save(&m).await.unwrap();

        delete_meeting(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&fs) as Arc<dyn MeetingFileStore>,
            Path::new("/rec"),
            &m.id,
            DeleteMode::Full,
        )
        .await
        .unwrap();

        assert!(mr.find_by_id(&m.id).await.unwrap().is_none());
        assert!(
            fs.removed_dirs.lock().unwrap().contains(&m.meeting_dir),
            "expected slug dir to be removed, got removed_dirs={:?}",
            fs.removed_dirs.lock().unwrap()
        );
    }

    #[tokio::test]
    async fn full_delete_keeps_foreign_dir_for_in_place_import() {
        let mr = FakeMeetingRepo::new();
        let fs = FakeMeetingFileStore::new();
        let mut m = Meeting::new("M".into(), PathBuf::from("/elsewhere/audio.m4a"));
        // In-place import points meeting_dir at a foreign folder.
        m.meeting_dir = PathBuf::from("/elsewhere");
        mr.save(&m).await.unwrap();

        delete_meeting(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&fs) as Arc<dyn MeetingFileStore>,
            Path::new("/rec"),
            &m.id,
            DeleteMode::Full,
        )
        .await
        .unwrap();

        assert!(mr.find_by_id(&m.id).await.unwrap().is_none());
        assert!(
            !fs.removed_dirs.lock().unwrap().contains(&m.meeting_dir),
            "foreign dir must not be removed",
        );
        assert!(fs
            .removed_files
            .lock()
            .unwrap()
            .contains(&PathBuf::from("/elsewhere/audio.m4a")));
    }
}
