mod meeting_repo;
mod job_repo;

pub use meeting_repo::SqliteMeetingRepo;
pub use job_repo::SqliteJobRepo;

use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use rusqlite::Connection;

const MIGRATION: &str = include_str!("../../../../migrations/001_initial.sql");

pub struct Db {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Arc<Self>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create db dir {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open sqlite at {}", path.display()))?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        conn.execute_batch(MIGRATION).context("apply migrations")?;

        Ok(Arc::new(Self { conn: Arc::new(Mutex::new(conn)) }))
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Arc<Self>> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(MIGRATION)?;
        Ok(Arc::new(Self { conn: Arc::new(Mutex::new(conn)) }))
    }
}
