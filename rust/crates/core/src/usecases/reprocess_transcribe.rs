use std::sync::Arc;
use crate::{CoreError, entities::Job, ports::{JobRepo, MeetingRepo}};

/// Re-transcribe an existing meeting: clear the stale transcript + protocol,
/// then enqueue a fresh transcription job (the worker re-runs Whisper).
pub async fn reprocess_transcribe(
    meeting_repo: Arc<dyn MeetingRepo>,
    job_repo: Arc<dyn JobRepo>,
    meeting_id: &str,
) -> Result<Job, CoreError> {
    let meeting = meeting_repo
        .find_by_id(meeting_id)
        .await?
        .ok_or_else(|| CoreError::NotFound(format!("meeting '{meeting_id}'")))?;

    if meeting.audio_path.as_os_str().is_empty() {
        return Err(CoreError::Validation(
            "cannot re-transcribe: audio was deleted".into(),
        ));
    }

    meeting_repo.clear_transcript(meeting_id).await?;
    meeting_repo.clear_protocol(meeting_id).await?;

    let job = Job::new_reprocess_transcribe(meeting_id.to_string());
    job_repo.enqueue(&job).await?;
    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Meeting;
    use crate::fakes::{FakeJobRepo, FakeMeetingRepo};
    use std::path::PathBuf;

    #[tokio::test]
    async fn clears_outputs_and_enqueues() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m = Meeting::new("M".into(), PathBuf::from("/a.wav"));
        mr.save(&m).await.unwrap();
        mr.save_transcript(&m.id, "old").await.unwrap();
        mr.save_protocol(&m.id, "old proto").await.unwrap();

        let job = reprocess_transcribe(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&jr) as Arc<dyn JobRepo>,
            &m.id,
        )
        .await
        .unwrap();

        assert!(job.kind.is_transcription());
        let after = mr.find_by_id(&m.id).await.unwrap().unwrap();
        assert!(after.transcript_text.is_none());
        assert!(after.protocol_text.is_none());
    }

    #[tokio::test]
    async fn missing_meeting_errors() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let err = reprocess_transcribe(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&jr) as Arc<dyn JobRepo>,
            "nope",
        )
        .await
        .unwrap_err();
        assert!(matches!(err, CoreError::NotFound(_)));
    }
}
