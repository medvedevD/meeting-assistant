use std::path::PathBuf;
use std::sync::Arc;
use crate::{CoreError, entities::{Job, Meeting}, ports::{JobRepo, MeetingRepo}};

pub async fn submit_transcription_job(
    meeting_repo: Arc<dyn MeetingRepo>,
    job_repo: Arc<dyn JobRepo>,
    audio_path: PathBuf,
    name: String,
) -> Result<Job, CoreError> {
    let meeting = Meeting::new(name, audio_path);
    meeting_repo.save(&meeting).await?;

    let job = Job::new_transcribe(meeting.id.clone());
    job_repo.enqueue(&job).await?;

    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entities::JobStatus, fakes::{FakeJobRepo, FakeMeetingRepo}};

    #[tokio::test]
    async fn creates_meeting_and_job() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();

        let job = submit_transcription_job(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&jr) as Arc<dyn JobRepo>,
            PathBuf::from("/audio/meeting.wav"),
            "Планёрка".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(job.status, JobStatus::Pending);
        assert!(!job.id.is_empty());
        assert!(!job.meeting_id.is_empty());

        let meeting = mr.find_by_id(&job.meeting_id).await.unwrap().unwrap();
        assert_eq!(meeting.name, "Планёрка");
        assert_eq!(meeting.audio_path, PathBuf::from("/audio/meeting.wav"));

        let stored_job = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(stored_job.meeting_id, job.meeting_id);
    }
}
