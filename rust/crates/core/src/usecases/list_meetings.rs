use std::sync::Arc;
use crate::{CoreError, entities::Meeting, ports::MeetingRepo};

pub async fn list_meetings(
    meeting_repo: Arc<dyn MeetingRepo>,
) -> Result<Vec<Meeting>, CoreError> {
    meeting_repo.list_all().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::{entities::Meeting, fakes::FakeMeetingRepo, ports::MeetingRepo};

    #[tokio::test]
    async fn empty_repo_returns_empty_vec() {
        let repo = FakeMeetingRepo::new();
        let meetings = list_meetings(Arc::clone(&repo) as Arc<dyn MeetingRepo>)
            .await
            .unwrap();
        assert!(meetings.is_empty());
    }

    #[tokio::test]
    async fn returns_all_saved_meetings() {
        let repo = FakeMeetingRepo::new();
        let m1 = Meeting::new("Планёрка".to_string(), PathBuf::from("/a.wav"));
        let m2 = Meeting::new("1-на-1".to_string(), PathBuf::from("/b.wav"));
        repo.save(&m1).await.unwrap();
        repo.save(&m2).await.unwrap();

        let meetings = list_meetings(Arc::clone(&repo) as Arc<dyn MeetingRepo>)
            .await
            .unwrap();

        assert_eq!(meetings.len(), 2);
        let names: Vec<_> = meetings.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"Планёрка"));
        assert!(names.contains(&"1-на-1"));
    }
}
