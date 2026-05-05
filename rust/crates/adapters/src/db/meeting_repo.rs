use std::path::PathBuf;
use std::sync::Arc;
use async_trait::async_trait;
use rusqlite::OptionalExtension;
use meeting_core::{CoreError, entities::Meeting, ports::MeetingRepo};
use super::Db;

pub struct SqliteMeetingRepo(pub Arc<Db>);

#[async_trait]
impl MeetingRepo for SqliteMeetingRepo {
    async fn save(&self, meeting: &Meeting) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = meeting.id.clone();
        let name = meeting.name.clone();
        let audio_path = meeting.audio_path.display().to_string();
        let created_at = meeting.created_at;

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO meetings (id, name, audio_path, created_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, name, audio_path, created_at],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Meeting>, CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.query_row(
                "SELECT id, name, audio_path, transcript_text, created_at FROM meetings WHERE id=?1",
                [&id],
                |row| {
                    Ok(Meeting {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        audio_path: PathBuf::from(row.get::<_, String>(2)?),
                        transcript_text: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )
            .optional()
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))
    }

    async fn save_transcript(&self, id: &str, text: &str) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();
        let text = text.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE meetings SET transcript_text=?1 WHERE id=?2",
                rusqlite::params![text, id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use meeting_core::entities::Meeting;
    use crate::db::Db;

    fn make_repo() -> SqliteMeetingRepo {
        let db = Db::open_in_memory().unwrap();
        SqliteMeetingRepo(db)
    }

    #[tokio::test]
    async fn save_and_find_roundtrip() {
        let repo = make_repo();
        let m = Meeting::new("Планёрка".to_string(), PathBuf::from("/audio/m.wav"));
        repo.save(&m).await.unwrap();

        let found = repo.find_by_id(&m.id).await.unwrap().unwrap();
        assert_eq!(found.id, m.id);
        assert_eq!(found.name, "Планёрка");
        assert_eq!(found.audio_path, PathBuf::from("/audio/m.wav"));
        assert!(found.transcript_text.is_none());
    }

    #[tokio::test]
    async fn find_unknown_returns_none() {
        let repo = make_repo();
        assert!(repo.find_by_id("no-such-id").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn save_transcript_persists() {
        let repo = make_repo();
        let m = Meeting::new("Встреча".to_string(), PathBuf::from("/audio/x.wav"));
        repo.save(&m).await.unwrap();
        repo.save_transcript(&m.id, "Привет мир").await.unwrap();

        let found = repo.find_by_id(&m.id).await.unwrap().unwrap();
        assert_eq!(found.transcript_text.as_deref(), Some("Привет мир"));
    }
}
