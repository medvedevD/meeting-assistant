use super::Db;
use async_trait::async_trait;
use meeting_core::{
    entities::{ErrorClass, Job, JobKind, JobStatus},
    ports::JobRepo,
    CoreError,
};
use rusqlite::OptionalExtension;
use std::sync::Arc;

pub struct SqliteJobRepo(pub Arc<Db>);

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    let kind_str: String = row.get(2)?;
    let status_str: String = row.get(3)?;
    Ok(Job {
        id: row.get(0)?,
        meeting_id: row.get(1)?,
        kind: JobKind::from_str(&kind_str).unwrap_or(JobKind::Transcribe),
        status: JobStatus::from_str(&status_str).unwrap_or(JobStatus::Pending),
        attempts: row.get::<_, i64>(4)? as u32,
        last_error: row.get(5)?,
        retry_after: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        template_name: row.get(9)?,
        // Guard for NULL (old rows) and unknown/legacy values.
        error_class: row
            .get::<_, Option<String>>(10)?
            .as_deref()
            .and_then(ErrorClass::from_str),
        then_protocol: row.get::<_, i64>(11)? != 0,
    })
}

const SELECT_COLS: &str =
    "id, meeting_id, kind, status, attempts, last_error, retry_after, created_at, updated_at, template_name, error_class, then_protocol";

#[async_trait]
impl JobRepo for SqliteJobRepo {
    async fn enqueue(&self, job: &Job) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = job.id.clone();
        let meeting_id = job.meeting_id.clone();
        let kind = job.kind.as_str().to_string();
        let status = job.status.as_str().to_string();
        let created_at = job.created_at;
        let updated_at = job.updated_at;
        let template_name = job.template_name.clone();
        let then_protocol = job.then_protocol as i64;

        let retry_after = job.retry_after;

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO jobs (id, meeting_id, kind, status, attempts, retry_after, created_at, updated_at, template_name, then_protocol)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![id, meeting_id, kind, status, retry_after, created_at, updated_at, template_name, then_protocol],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Job>, CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.query_row(
                &format!("SELECT {SELECT_COLS} FROM jobs WHERE id=?1"),
                [&id],
                row_to_job,
            )
            .optional()
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))
    }

    async fn list_active(&self) -> Result<Vec<Job>, CoreError> {
        let db = Arc::clone(&self.0);

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLS} FROM jobs
                 WHERE status IN ('pending', 'running')
                 ORDER BY created_at"
            ))?;
            let jobs = stmt
                .query_map([], row_to_job)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok::<_, rusqlite::Error>(jobs)
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))
    }

    async fn claim_pending_kind(
        &self,
        kinds: &[JobKind],
        now_ts: i64,
    ) -> Result<Option<Job>, CoreError> {
        if kinds.is_empty() {
            return Ok(None);
        }
        let db = Arc::clone(&self.0);
        let kind_strs: Vec<String> = kinds.iter().map(|k| k.as_str().to_string()).collect();

        tokio::task::spawn_blocking(move || {
            let mut conn = db.conn.lock().unwrap();
            let tx = conn.transaction()?;

            // Build `?2, ?3, ?4, …` for the kind IN-list (rusqlite has no
            // dynamic IN sugar without the `rarray` feature; the list is at
            // most 3 entries, so an inline build is the simplest path).
            let placeholders: String = (0..kind_strs.len())
                .map(|i| format!("?{}", i + 2))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT {SELECT_COLS} FROM jobs
                 WHERE status='pending' AND retry_after <= ?1
                   AND kind IN ({placeholders})
                 ORDER BY created_at LIMIT 1"
            );

            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(1 + kind_strs.len());
            params.push(Box::new(now_ts));
            for k in &kind_strs {
                params.push(Box::new(k.clone()));
            }
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

            let mut job = tx
                .query_row(&sql, param_refs.as_slice(), row_to_job)
                .optional()?;

            if let Some(ref mut j) = job {
                tx.execute(
                    "UPDATE jobs SET status='running', updated_at=?1 WHERE id=?2",
                    rusqlite::params![now_ts, j.id],
                )?;
                j.status = JobStatus::Running;
                j.updated_at = now_ts;
            }

            tx.commit()?;
            Ok::<_, rusqlite::Error>(job)
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))
    }

    async fn mark_done(&self, id: &str, now_ts: i64) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE jobs SET status='done', updated_at=?1 WHERE id=?2",
                rusqlite::params![now_ts, id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn reset_for_retry(
        &self,
        id: &str,
        error: &str,
        attempts: u32,
        retry_after: i64,
        now_ts: i64,
    ) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();
        let error = error.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE jobs SET status='pending', attempts=?1, last_error=?2,
                 retry_after=?3, updated_at=?4 WHERE id=?5",
                rusqlite::params![attempts, error, retry_after, now_ts, id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn mark_permanently_failed(
        &self,
        id: &str,
        error: &str,
        error_class: Option<&str>,
        attempts: u32,
        now_ts: i64,
    ) -> Result<(), CoreError> {
        let db = Arc::clone(&self.0);
        let id = id.to_string();
        let error = error.to_string();
        let error_class = error_class.map(|s| s.to_string());

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE jobs SET status='failed', attempts=?1, last_error=?2,
                 error_class=?3, updated_at=?4 WHERE id=?5",
                rusqlite::params![attempts, error, error_class, now_ts, id],
            )
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn recover_running_jobs(&self, now_ts: i64) -> Result<u64, CoreError> {
        let db = Arc::clone(&self.0);

        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().unwrap();
            // Reset stuck `running` jobs back to `pending`, incrementing the attempt counter
            // so repeated crashes eventually exhaust retries instead of looping forever.
            let n = conn.execute(
                "UPDATE jobs SET status='pending', attempts=attempts+1,
                 last_error='interrupted by process restart', updated_at=?1
                 WHERE status='running'",
                rusqlite::params![now_ts],
            )?;
            Ok::<_, rusqlite::Error>(n as u64)
        })
        .await
        .map_err(|e| CoreError::Storage(e.to_string()))?
        .map_err(|e| CoreError::Storage(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Db, SqliteMeetingRepo};
    use meeting_core::{
        entities::{Job, JobStatus, Meeting},
        ports::MeetingRepo,
    };
    use std::path::PathBuf;

    fn make_repos() -> (SqliteMeetingRepo, SqliteJobRepo) {
        let db = Db::open_in_memory().unwrap();
        (SqliteMeetingRepo(Arc::clone(&db)), SqliteJobRepo(db))
    }

    async fn insert_meeting(mr: &SqliteMeetingRepo) -> Meeting {
        let m = Meeting::new("Test".to_string(), PathBuf::from("/a.wav"));
        mr.save(&m).await.unwrap();
        m
    }

    #[tokio::test]
    async fn enqueue_and_find() {
        let (mr, jr) = make_repos();
        let m = insert_meeting(&mr).await;
        let job = Job::new_transcribe(m.id.clone());
        jr.enqueue(&job).await.unwrap();

        let found = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(found.id, job.id);
        assert_eq!(found.meeting_id, m.id);
        assert_eq!(found.status, JobStatus::Pending);
        assert_eq!(found.attempts, 0);
    }

    #[tokio::test]
    async fn find_unknown_returns_none() {
        let (_, jr) = make_repos();
        assert!(jr.find_by_id("ghost").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn claim_pending_marks_running() {
        let (mr, jr) = make_repos();
        let m = insert_meeting(&mr).await;
        let job = Job::new_transcribe(m.id);
        jr.enqueue(&job).await.unwrap();

        let claimed = jr.claim_pending(i64::MAX).await.unwrap().unwrap();
        assert_eq!(claimed.id, job.id);
        assert_eq!(claimed.status, JobStatus::Running);

        let in_db = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(in_db.status, JobStatus::Running);
    }

    #[tokio::test]
    async fn claim_pending_returns_none_when_queue_empty() {
        let (_, jr) = make_repos();
        assert!(jr.claim_pending(i64::MAX).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn claim_pending_respects_retry_after() {
        let (mr, jr) = make_repos();
        let m = insert_meeting(&mr).await;
        let mut job = Job::new_transcribe(m.id);
        job.retry_after = 9999999999; // far future
        jr.enqueue(&job).await.unwrap();

        // now_ts < retry_after → not eligible
        assert!(jr.claim_pending(0).await.unwrap().is_none());
        // now_ts >= retry_after → eligible
        assert!(jr.claim_pending(i64::MAX).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn mark_done_changes_status() {
        let (mr, jr) = make_repos();
        let m = insert_meeting(&mr).await;
        let job = Job::new_transcribe(m.id);
        jr.enqueue(&job).await.unwrap();
        jr.claim_pending(i64::MAX).await.unwrap();

        jr.mark_done(&job.id, 1000).await.unwrap();
        let j = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Done);
        assert_eq!(j.updated_at, 1000);
    }

    #[tokio::test]
    async fn reset_for_retry_resets_to_pending() {
        let (mr, jr) = make_repos();
        let m = insert_meeting(&mr).await;
        let job = Job::new_transcribe(m.id);
        jr.enqueue(&job).await.unwrap();
        jr.claim_pending(i64::MAX).await.unwrap();

        jr.reset_for_retry(&job.id, "timeout", 1, 5000, 1000)
            .await
            .unwrap();
        let j = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Pending);
        assert_eq!(j.attempts, 1);
        assert_eq!(j.last_error.as_deref(), Some("timeout"));
        assert_eq!(j.retry_after, 5000);
    }

    #[tokio::test]
    async fn mark_permanently_failed_sets_failed_status() {
        let (mr, jr) = make_repos();
        let m = insert_meeting(&mr).await;
        let job = Job::new_transcribe(m.id);
        jr.enqueue(&job).await.unwrap();
        jr.claim_pending(i64::MAX).await.unwrap();

        jr.mark_permanently_failed(&job.id, "crashed", Some("api_auth"), 5, 2000)
            .await
            .unwrap();
        let j = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Failed);
        assert_eq!(j.attempts, 5);
        assert_eq!(j.last_error.as_deref(), Some("crashed"));
        assert_eq!(
            j.error_class,
            Some(meeting_core::entities::ErrorClass::ApiAuth)
        );
    }

    #[tokio::test]
    async fn list_active_returns_pending_and_running_oldest_first() {
        let (mr, jr) = make_repos();
        let m = insert_meeting(&mr).await;

        // pending (created first), running (claimed), done, failed.
        let mut pending = Job::new_transcribe(m.id.clone());
        pending.created_at = 100;
        jr.enqueue(&pending).await.unwrap();

        let mut running = Job::new_transcribe(m.id.clone());
        running.created_at = 200;
        jr.enqueue(&running).await.unwrap();
        jr.claim_pending(i64::MAX).await.unwrap(); // claims oldest pending → `pending`

        let mut done = Job::new_transcribe(m.id.clone());
        done.created_at = 300;
        jr.enqueue(&done).await.unwrap();
        jr.mark_done(&done.id, 1000).await.unwrap();

        let mut failed = Job::new_transcribe(m.id.clone());
        failed.created_at = 400;
        jr.enqueue(&failed).await.unwrap();
        jr.mark_permanently_failed(&failed.id, "boom", None, 5, 1000)
            .await
            .unwrap();

        let active = jr.list_active().await.unwrap();
        let statuses: Vec<JobStatus> = active.iter().map(|j| j.status.clone()).collect();
        // `claim_pending` flipped the first-created job to running; the
        // second stays pending. done/failed are excluded. Order is by created_at.
        assert_eq!(active.len(), 2);
        assert_eq!(statuses, vec![JobStatus::Running, JobStatus::Pending]);
        assert!(active[0].created_at <= active[1].created_at);
    }

    #[tokio::test]
    async fn list_active_empty_when_no_jobs() {
        let (_, jr) = make_repos();
        assert!(jr.list_active().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn error_class_null_for_pending_and_persists_on_failure() {
        let (mr, jr) = make_repos();
        let m = insert_meeting(&mr).await;
        let job = Job::new_transcribe(m.id);
        jr.enqueue(&job).await.unwrap();

        // Old/pending rows have NULL error_class → None, no panic.
        let pending = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert!(pending.error_class.is_none());

        jr.mark_permanently_failed(&job.id, "boom", None, 5, 1)
            .await
            .unwrap();
        let failed = jr.find_by_id(&job.id).await.unwrap().unwrap();
        assert!(failed.error_class.is_none());
    }
}
