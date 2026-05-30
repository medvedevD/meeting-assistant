use crate::{entities::Job, ports::JobRepo, CoreError};
use std::sync::Arc;

pub async fn get_job_status(
    job_repo: Arc<dyn JobRepo>,
    job_id: &str,
) -> Result<Option<Job>, CoreError> {
    job_repo.find_by_id(job_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Job;
    use crate::fakes::FakeJobRepo;

    #[tokio::test]
    async fn returns_none_for_unknown_id() {
        let jr = FakeJobRepo::new();
        let result = get_job_status(jr, "no-such-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_job_for_known_id() {
        let jr = FakeJobRepo::new();
        let job = Job::new_transcribe("meeting-123".to_string());
        jr.enqueue(&job).await.unwrap();

        let found = get_job_status(Arc::clone(&jr) as Arc<dyn JobRepo>, &job.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, job.id);
        assert_eq!(found.meeting_id, "meeting-123");
    }
}
