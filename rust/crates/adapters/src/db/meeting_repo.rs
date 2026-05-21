use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use rusqlite::OptionalExtension;
use meeting_core::{CoreError, entities::Meeting, ports::MeetingRepo};
use super::Db;

pub struct SqliteMeetingRepo(pub Arc<Db>);

fn row_to_meeting(row: &rusqlite::Row) -> rusqlite::Result<Meeting> {
    let audio_path = PathBuf::from(row.get::<_, String>(2)?);
    let meeting_dir = audio_path.parent()
        .map(PathBuf::from)
        .unwrap_or_default();
    Ok(Meeting {
        id:              row.get(0)?,
        name:            row.get(1)?,
        audio_path,
        meeting_dir,
        transcript_text: row.get(3)?,
        protocol_text:   row.get(4)?,
        transcript_path: row.get::<_, Option<String>>(5)?.map(PathBuf::from),
        protocol_path:   row.get::<_, Option<String>>(6)?.map(PathBuf::from),
        created_at:      row.get(7)?,
    })
}

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
                "SELECT id, name, audio_path, transcript_text, protocol_text, \
                        transcript_path, protocol_path, created_at \
                   FROM meetings WHERE id=?1",
                [&id],
                row_to_meeting,
            )
            .optional()
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))
    }

    async fn find_by_audio_path(&self, path: &Path) -> Result<Option<Meeting>, CoreError> {
        let db = Arc::clone(&self.0);
        let path_str = path.display().to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.query_row(
                "SELECT id, name, audio_path, transcript_text, protocol_text, \
                        transcript_path, protocol_path, created_at \
                   FROM meetings WHERE audio_path=?1 ORDER BY created_at DESC LIMIT 1",
                [&path_str],
                row_to_meeting,
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

    async fn save_protocol(&self, id: &str, text: &str) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();
        let text = text.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE meetings SET protocol_text=?1 WHERE id=?2",
                rusqlite::params![text, id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn save_transcript_file(&self, id: &str, text: &str, path: &Path) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();
        let text = text.to_string();
        let path_str = path.display().to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE meetings SET transcript_text=?1, transcript_path=?2 WHERE id=?3",
                rusqlite::params![text, path_str, id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn save_protocol_file(&self, id: &str, text: &str, path: &Path) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();
        let text = text.to_string();
        let path_str = path.display().to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE meetings SET protocol_text=?1, protocol_path=?2 WHERE id=?3",
                rusqlite::params![text, path_str, id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn update_name(&self, id: &str, name: &str) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();
        let name = name.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE meetings SET name=?1 WHERE id=?2",
                rusqlite::params![name, id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn delete_audio_only(&self, id: &str) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            // audio_path is NOT NULL; an empty string is the "audio removed"
            // sentinel (see Phase 3 deviation note). Transcript/protocol kept.
            conn.execute(
                "UPDATE meetings SET audio_path='' WHERE id=?1",
                rusqlite::params![id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn clear_transcript(&self, id: &str) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE meetings SET transcript_text=NULL, transcript_path=NULL WHERE id=?1",
                rusqlite::params![id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn clear_protocol(&self, id: &str) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE meetings SET protocol_text=NULL, protocol_path=NULL WHERE id=?1",
                rusqlite::params![id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let mut conn = db.conn.lock().unwrap();
            let tx = conn.transaction()?;
            // jobs reference meetings(id); remove children first to satisfy FK.
            tx.execute("DELETE FROM jobs WHERE meeting_id=?1", rusqlite::params![id])?;
            tx.execute("DELETE FROM meetings WHERE id=?1", rusqlite::params![id])?;
            tx.commit()
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<Meeting>, CoreError> {
        let db = Arc::clone(&self.0);

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    // Select presence flags only — avoids loading MB of text on every list call.
                    // transcript_text/protocol_text will be Some("") when present, None otherwise.
                    "SELECT id, name, audio_path, \
                       CASE WHEN transcript_text IS NOT NULL THEN '' ELSE NULL END, \
                       CASE WHEN protocol_text IS NOT NULL THEN '' ELSE NULL END, \
                       transcript_path, protocol_path, created_at \
                     FROM meetings ORDER BY created_at DESC",
                )
                .map_err(|e| CoreError::Storage(e.to_string()))?;

            let rows = stmt
                .query_map([], row_to_meeting)
                .map_err(|e| CoreError::Storage(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| CoreError::Storage(e.to_string()))?;

            Ok(rows)
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
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

    #[tokio::test]
    async fn save_transcript_file_persists_text_and_path() {
        let repo = make_repo();
        let m = Meeting::new("Встреча".to_string(), PathBuf::from("/audio/x.wav"));
        repo.save(&m).await.unwrap();
        let path = PathBuf::from("/meetings/2026-05-10_14-30_abc/transcript.md");
        repo.save_transcript_file(&m.id, "Привет мир", &path).await.unwrap();

        let found = repo.find_by_id(&m.id).await.unwrap().unwrap();
        assert_eq!(found.transcript_text.as_deref(), Some("Привет мир"));
        assert_eq!(found.transcript_path, Some(path));
    }

    #[tokio::test]
    async fn save_protocol_file_persists_text_and_path() {
        let repo = make_repo();
        let m = Meeting::new("Встреча".to_string(), PathBuf::from("/audio/x.wav"));
        repo.save(&m).await.unwrap();
        let path = PathBuf::from("/meetings/2026-05-10_14-30_abc/protocol.md");
        repo.save_protocol_file(&m.id, "# Протокол", &path).await.unwrap();

        let found = repo.find_by_id(&m.id).await.unwrap().unwrap();
        assert_eq!(found.protocol_text.as_deref(), Some("# Протокол"));
        assert_eq!(found.protocol_path, Some(path));
    }

    #[tokio::test]
    async fn update_name_persists() {
        let repo = make_repo();
        let m = Meeting::new("Старое имя".to_string(), PathBuf::from("/audio/y.wav"));
        repo.save(&m).await.unwrap();
        repo.update_name(&m.id, "Новое имя").await.unwrap();

        let found = repo.find_by_id(&m.id).await.unwrap().unwrap();
        assert_eq!(found.name, "Новое имя");
    }

    #[tokio::test]
    async fn list_all_returns_all_in_desc_order() {
        let repo = make_repo();
        let m1 = Meeting::new("Первая".to_string(), PathBuf::from("/a.wav"));
        let m2 = Meeting::new("Вторая".to_string(), PathBuf::from("/b.wav"));
        repo.save(&m1).await.unwrap();
        repo.save(&m2).await.unwrap();

        let meetings = repo.list_all().await.unwrap();
        assert_eq!(meetings.len(), 2);
        let names: Vec<_> = meetings.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"Первая"));
        assert!(names.contains(&"Вторая"));
    }

    #[tokio::test]
    async fn list_all_empty() {
        let repo = make_repo();
        let meetings = repo.list_all().await.unwrap();
        assert!(meetings.is_empty());
    }

    #[tokio::test]
    async fn delete_audio_only_keeps_transcript_and_protocol() {
        let repo = make_repo();
        let m = Meeting::new("Встреча".to_string(), PathBuf::from("/audio/x.wav"));
        repo.save(&m).await.unwrap();
        repo.save_transcript(&m.id, "Текст").await.unwrap();
        repo.save_protocol(&m.id, "# Протокол").await.unwrap();

        repo.delete_audio_only(&m.id).await.unwrap();

        let found = repo.find_by_id(&m.id).await.unwrap().unwrap();
        assert_eq!(found.audio_path, PathBuf::new());
        assert_eq!(found.transcript_text.as_deref(), Some("Текст"));
        assert_eq!(found.protocol_text.as_deref(), Some("# Протокол"));
    }

    #[tokio::test]
    async fn clear_transcript_and_protocol_null_out() {
        let repo = make_repo();
        let m = Meeting::new("Встреча".to_string(), PathBuf::from("/audio/x.wav"));
        repo.save(&m).await.unwrap();
        repo.save_transcript_file(&m.id, "t", &PathBuf::from("/d/transcript.md")).await.unwrap();
        repo.save_protocol_file(&m.id, "p", &PathBuf::from("/d/protocol.md")).await.unwrap();

        repo.clear_transcript(&m.id).await.unwrap();
        repo.clear_protocol(&m.id).await.unwrap();

        let found = repo.find_by_id(&m.id).await.unwrap().unwrap();
        assert!(found.transcript_text.is_none());
        assert!(found.transcript_path.is_none());
        assert!(found.protocol_text.is_none());
        assert!(found.protocol_path.is_none());
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let repo = make_repo();
        let m = Meeting::new("Встреча".to_string(), PathBuf::from("/audio/x.wav"));
        repo.save(&m).await.unwrap();
        repo.delete(&m.id).await.unwrap();
        assert!(repo.find_by_id(&m.id).await.unwrap().is_none());
    }
}
