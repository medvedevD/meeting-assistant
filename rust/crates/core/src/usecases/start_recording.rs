use crate::{
    entities::{now_unix, Meeting},
    ports::{AudioCapture, CaptureSpec, MeetingRepo, ResolvedDevices},
    CoreError,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Start a new recording session.
///
/// Creates a `Meeting` record immediately, starts the audio capture session for
/// `spec` (source + echo-cancel + optional pinned devices), and persists the
/// meeting to the repo. Returns the saved `Meeting` together with the
/// [`ResolvedDevices`] actually opened, so the caller can show the user exactly
/// which mic / system source is live.
pub async fn start_recording(
    capture: Arc<dyn AudioCapture>,
    meeting_repo: Arc<dyn MeetingRepo>,
    meetings_dir: &Path,
    name: Option<String>,
    spec: CaptureSpec,
) -> Result<(Meeting, ResolvedDevices), CoreError> {
    let ts = now_unix();
    let meeting_name = name.unwrap_or_else(|| chrono_name(ts));
    let meeting = Meeting::new(meeting_name, PathBuf::new());

    let slug = slug_from_meeting(ts, &meeting.id[..8]);
    let meeting_dir = meetings_dir.join(&slug);

    // Create meeting directory immediately so the file is accessible on disk.
    std::fs::create_dir_all(&meeting_dir).map_err(|e| {
        CoreError::Storage(format!(
            "cannot create meeting dir {}: {e}",
            meeting_dir.display()
        ))
    })?;

    let audio_path = meeting_dir.join("recording.wav");
    let meeting = Meeting {
        meeting_dir,
        audio_path: audio_path.clone(),
        ..meeting
    };

    let resolved = capture
        .start_session(&meeting.id, &audio_path, spec)
        .await?;
    meeting_repo.save(&meeting).await?;
    Ok((meeting, resolved))
}

fn slug_from_meeting(ts: i64, uuid_prefix: &str) -> String {
    let mins = ts / 60 % 60;
    let hours = ts / 3600 % 24;
    let days = ts / 86400;
    let (y, m, d) = epoch_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}_{hours:02}-{mins:02}_{uuid_prefix}")
}

fn chrono_name(ts: i64) -> String {
    let mins = ts / 60 % 60;
    let hours = ts / 3600 % 24;
    let days = ts / 86400;
    let (y, m, d) = epoch_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02} {hours:02}:{mins:02}")
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
    use crate::fakes::{FakeAudioCapture, FakeMeetingRepo};
    use crate::ports::CaptureSource;
    use std::path::PathBuf;

    fn meetings_dir() -> PathBuf {
        PathBuf::from("/tmp/meetings")
    }

    async fn start(
        capture: Arc<FakeAudioCapture>,
        repo: Arc<FakeMeetingRepo>,
        name: Option<&str>,
        source: CaptureSource,
    ) -> Meeting {
        start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &meetings_dir(),
            name.map(str::to_string),
            CaptureSpec {
                source,
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .0
    }

    #[tokio::test]
    async fn creates_meeting_with_correct_audio_path() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let meeting = start(
            Arc::clone(&capture),
            Arc::clone(&repo),
            Some("Планёрка"),
            CaptureSource::Mic,
        )
        .await;

        assert_eq!(meeting.name, "Планёрка");
        assert_eq!(
            meeting.audio_path,
            meeting.meeting_dir.join("recording.wav")
        );
        assert!(meeting.audio_path.starts_with(meetings_dir()));
    }

    #[tokio::test]
    async fn meeting_dir_is_inside_meetings_dir() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let meeting = start(
            Arc::clone(&capture),
            Arc::clone(&repo),
            None,
            CaptureSource::Mic,
        )
        .await;

        assert!(meeting.meeting_dir.starts_with(meetings_dir()));
    }

    #[tokio::test]
    async fn starts_capture_session_for_meeting_id() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let meeting = start(
            Arc::clone(&capture),
            Arc::clone(&repo),
            None,
            CaptureSource::Mic,
        )
        .await;

        assert!(capture.is_active(&meeting.id));
        let started = capture.started.lock().unwrap();
        assert_eq!(started[0].0, meeting.id);
    }

    #[tokio::test]
    async fn persists_meeting_to_repo() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let meeting = start(
            Arc::clone(&capture),
            Arc::clone(&repo),
            Some("1-on-1"),
            CaptureSource::Mic,
        )
        .await;

        let found = repo.find_by_id(&meeting.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "1-on-1");
    }

    #[tokio::test]
    async fn default_name_is_non_empty() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let meeting = start(
            Arc::clone(&capture),
            Arc::clone(&repo),
            None,
            CaptureSource::Mic,
        )
        .await;

        assert!(!meeting.name.is_empty());
    }

    // ── CaptureSource propagation ─────────────────────────────────────────────

    #[tokio::test]
    async fn system_source_is_passed_to_capture_adapter() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        start(
            Arc::clone(&capture),
            Arc::clone(&repo),
            None,
            CaptureSource::System,
        )
        .await;

        assert_eq!(capture.last_source(), Some(CaptureSource::System));
    }

    #[tokio::test]
    async fn mixed_source_is_passed_to_capture_adapter() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        start(
            Arc::clone(&capture),
            Arc::clone(&repo),
            None,
            CaptureSource::Mixed,
        )
        .await;

        assert_eq!(capture.last_source(), Some(CaptureSource::Mixed));
    }

    #[tokio::test]
    async fn mic_is_default_source() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        start(
            Arc::clone(&capture),
            Arc::clone(&repo),
            None,
            CaptureSource::default(),
        )
        .await;

        assert_eq!(capture.last_source(), Some(CaptureSource::Mic));
    }

    // ── echo_cancel propagation ───────────────────────────────────────────────

    #[tokio::test]
    async fn echo_cancel_true_is_passed_to_capture_adapter() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &meetings_dir(),
            None,
            CaptureSpec {
                source: CaptureSource::Mixed,
                echo_cancel: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(capture.last_echo_cancel(), Some(true));
    }

    #[tokio::test]
    async fn echo_cancel_false_is_passed_to_capture_adapter() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &meetings_dir(),
            None,
            CaptureSpec {
                source: CaptureSource::Mixed,
                echo_cancel: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(capture.last_echo_cancel(), Some(false));
    }

    // ── device selection ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn pinned_devices_are_passed_to_capture_adapter() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &meetings_dir(),
            None,
            CaptureSpec {
                source: CaptureSource::Mixed,
                echo_cancel: false,
                mic_device: Some("USB Mic".into()),
                system_device: Some("Speakers.monitor".into()),
            },
        )
        .await
        .unwrap();

        let spec = capture.last_spec().unwrap();
        assert_eq!(spec.mic_device.as_deref(), Some("USB Mic"));
        assert_eq!(spec.system_device.as_deref(), Some("Speakers.monitor"));
    }

    #[tokio::test]
    async fn resolved_devices_are_returned_to_caller() {
        let capture = FakeAudioCapture::new();
        let repo = FakeMeetingRepo::new();

        let (_meeting, resolved) = start_recording(
            Arc::clone(&capture) as Arc<dyn AudioCapture>,
            Arc::clone(&repo) as Arc<dyn MeetingRepo>,
            &meetings_dir(),
            None,
            CaptureSpec {
                source: CaptureSource::Mic,
                mic_device: Some("USB Mic".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(resolved.mic.as_deref(), Some("USB Mic"));
        assert_eq!(resolved.system, None);
    }

    // ── slug format ───────────────────────────────────────────────────────────

    #[test]
    fn slug_format_is_correct() {
        // 2026-05-10 14:30 UTC
        let ts = 1778423400i64;
        let slug = slug_from_meeting(ts, "a1b2c3d4");
        assert_eq!(slug, "2026-05-10_14-30_a1b2c3d4");
    }
}
