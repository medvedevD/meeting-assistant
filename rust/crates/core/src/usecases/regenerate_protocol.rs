use std::sync::Arc;
use crate::{CoreError, entities::Job, ports::{JobRepo, MeetingRepo}};

/// Regenerate the protocol for an existing meeting from its stored transcript,
/// optionally with a different template. Clears the old protocol and enqueues a
/// `RegenerateProtocol` job; the worker performs the LLM call.
pub async fn regenerate_protocol(
    meeting_repo: Arc<dyn MeetingRepo>,
    job_repo: Arc<dyn JobRepo>,
    meeting_id: &str,
    template_name: Option<String>,
) -> Result<Job, CoreError> {
    let meeting = meeting_repo
        .find_by_id(meeting_id)
        .await?
        .ok_or_else(|| CoreError::NotFound(format!("meeting '{meeting_id}'")))?;

    match meeting.transcript_text {
        Some(t) if !t.is_empty() => {}
        _ => {
            return Err(CoreError::Validation(
                "cannot regenerate protocol: meeting has no transcript".into(),
            ));
        }
    }

    meeting_repo.clear_protocol(meeting_id).await?;

    let job = Job::new_regenerate_protocol(meeting_id.to_string(), template_name);
    job_repo.enqueue(&job).await?;
    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{JobKind, Meeting};
    use crate::fakes::{FakeJobRepo, FakeMeetingRepo};
    use std::path::PathBuf;

    #[tokio::test]
    async fn enqueues_with_template_when_transcript_present() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m = Meeting::new("M".into(), PathBuf::from("/a.wav"));
        mr.save(&m).await.unwrap();
        mr.save_transcript(&m.id, "some transcript").await.unwrap();

        let job = regenerate_protocol(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&jr) as Arc<dyn JobRepo>,
            &m.id,
            Some("1-на-1".into()),
        )
        .await
        .unwrap();

        assert_eq!(job.kind, JobKind::RegenerateProtocol);
        assert_eq!(job.template_name.as_deref(), Some("1-на-1"));
    }

    #[tokio::test]
    async fn errors_without_transcript() {
        let mr = FakeMeetingRepo::new();
        let jr = FakeJobRepo::new();
        let m = Meeting::new("M".into(), PathBuf::from("/a.wav"));
        mr.save(&m).await.unwrap();

        let err = regenerate_protocol(
            Arc::clone(&mr) as Arc<dyn MeetingRepo>,
            Arc::clone(&jr) as Arc<dyn JobRepo>,
            &m.id,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }
}
