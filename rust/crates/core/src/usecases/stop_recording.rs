use std::sync::Arc;
use crate::{
    CoreError,
    entities::Meeting,
    ports::{AudioCapture, MeetingRepo},
};

/// Stop an active recording session and return the persisted `Meeting`.
///
/// Stops the capture (which finalizes the WAV file) then fetches the meeting
/// from the repo so the caller has the up-to-date record.
pub async fn stop_recording(
    capture: Arc<dyn AudioCapture>,
    meeting_repo: Arc<dyn MeetingRepo>,
    meeting_id: &str,
) -> Result<Meeting, CoreError> {
    capture.stop_session(meeting_id).await?;

    meeting_repo
        .find_by_id(meeting_id)
        .await?
        .ok_or_else(|| CoreError::NotFound(meeting_id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::{
        entities::Meeting,
        fakes::{FakeAudioCapture, FakeMeetingRepo},
        ports::{CaptureSource, MeetingRepo},
    };

    async fn seeded_repo(name: &str) -> (Arc<FakeMeetingRepo>, Meeting) {
        let repo = FakeMeetingRepo::new();
        let meeting = Meeting::new(name.to_string(), PathBuf::from("/tmp/r.wav"));
        repo.save(&meeting).await.unwrap();
        (repo, meeting)
    }

    #[tokio::test]
    async fn stops_capture_and_returns_meeting() {
        let capture = FakeAudioCapture::new();
        let (repo, meeting) = seeded_repo("Дейлик").await;

        // Simulate an active session
        capture
            .start_session(&meeting.id, std::path::Path::new("/tmp/r.wav"), CaptureSource::Mic, false)
            .await
            .unwrap();

        let result = stop_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &meeting.id,
        )
        .await
        .unwrap();

        assert_eq!(result.id, meeting.id);
        assert_eq!(result.name, "Дейлик");
        assert!(!capture.is_active(&meeting.id));
    }

    #[tokio::test]
    async fn session_not_active_returns_err() {
        let capture = FakeAudioCapture::new();
        let (repo, meeting) = seeded_repo("X").await;

        let err = stop_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &meeting.id,
        )
        .await;

        assert!(matches!(err, Err(CoreError::Recording(_))));
    }

    #[tokio::test]
    async fn meeting_not_in_repo_returns_not_found() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();
        let id = "ghost-id";

        // manually activate a session that has no DB record
        capture
            .start_session(id, std::path::Path::new("/tmp/x.wav"), CaptureSource::Mic, false)
            .await
            .unwrap();

        let err = stop_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            id,
        )
        .await;

        assert!(matches!(err, Err(CoreError::NotFound(_))));
    }
}
