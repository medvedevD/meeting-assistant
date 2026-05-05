use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::{
    CoreError,
    entities::{Meeting, now_unix},
    ports::{AudioCapture, MeetingRepo},
};

/// Start a new mic recording session.
///
/// Creates a `Meeting` record immediately (audio_path points to the future file),
/// starts the audio capture session, and persists the meeting to the repo.
/// Returns the saved `Meeting` so the caller has the session id.
pub async fn start_recording(
    capture: Arc<dyn AudioCapture>,
    meeting_repo: Arc<dyn MeetingRepo>,
    recordings_dir: &Path,
    name: Option<String>,
) -> Result<Meeting, CoreError> {
    let meeting_name = name.unwrap_or_else(|| {
        chrono_name(now_unix())
    });
    let meeting = Meeting::new(meeting_name, PathBuf::new()); // placeholder; overwrite below
    let audio_path = recordings_dir.join(&meeting.id).join("recording.wav");

    let meeting = Meeting {
        audio_path: audio_path.clone(),
        ..meeting
    };

    capture.start_session(&meeting.id, &audio_path).await?;
    meeting_repo.save(&meeting).await?;
    Ok(meeting)
}

fn chrono_name(ts: i64) -> String {
    // Simple: "Meeting 2026-05-05T14:03"
    let secs = ts;
    let mins = secs / 60 % 60;
    let hours = secs / 3600 % 24;
    let days = secs / 86400;
    // Approximate Gregorian date from epoch (good enough for a default name)
    let (y, m, d) = epoch_to_ymd(days);
    format!("Meeting {y:04}-{m:02}-{d:02}T{hours:02}:{mins:02}")
}

fn epoch_to_ymd(days: i64) -> (i64, i64, i64) {
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::fakes::{FakeAudioCapture, FakeMeetingRepo};

    fn recordings_dir() -> PathBuf {
        PathBuf::from("/tmp/recordings")
    }

    #[tokio::test]
    async fn creates_meeting_with_correct_audio_path() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();
        let dir = recordings_dir();

        let meeting = start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &dir,
            Some("Планёрка".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(meeting.name, "Планёрка");
        let expected = dir.join(&meeting.id).join("recording.wav");
        assert_eq!(meeting.audio_path, expected);
    }

    #[tokio::test]
    async fn starts_capture_session_for_meeting_id() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let meeting = start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &recordings_dir(),
            None,
        )
        .await
        .unwrap();

        assert!(capture.is_active(&meeting.id));
        let started = capture.started.lock().unwrap();
        assert_eq!(started.as_slice(), [meeting.id.as_str()]);
    }

    #[tokio::test]
    async fn persists_meeting_to_repo() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let meeting = start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &recordings_dir(),
            Some("1-on-1".to_string()),
        )
        .await
        .unwrap();

        let found = repo.find_by_id(&meeting.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "1-on-1");
    }

    #[tokio::test]
    async fn default_name_is_non_empty() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let meeting = start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &recordings_dir(),
            None,
        )
        .await
        .unwrap();

        assert!(!meeting.name.is_empty());
    }
}
