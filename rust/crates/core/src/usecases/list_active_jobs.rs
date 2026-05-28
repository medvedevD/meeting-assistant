use crate::{entities::Job, ports::JobRepo, CoreError};
use std::sync::Arc;

/// In-flight jobs (`pending` or `running`), oldest first. Seeds the UI's
/// active-jobs view after an app restart.
pub async fn list_active_jobs(job_repo: Arc<dyn JobRepo>) -> Result<Vec<Job>, CoreError> {
    job_repo.list_active().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Job;
    use crate::fakes::FakeJobRepo;

    #[tokio::test]
    async fn returns_only_in_flight_jobs() {
        let jr = FakeJobRepo::new();
        let pending = Job::new_transcribe("m1".to_string());
        jr.enqueue(&pending).await.unwrap();
        let done = Job::new_transcribe("m2".to_string());
        jr.enqueue(&done).await.unwrap();
        jr.mark_done(&done.id, 1).await.unwrap();

        let active = list_active_jobs(Arc::clone(&jr) as Arc<dyn JobRepo>)
            .await
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, pending.id);
    }
}
