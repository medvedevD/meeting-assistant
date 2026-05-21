mod meeting_repo;
mod job_repo;

pub use meeting_repo::SqliteMeetingRepo;
pub use job_repo::SqliteJobRepo;

use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use rusqlite::Connection;

// Each entry is (version, sql). Applied in order; already-applied versions are skipped.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../../../migrations/001_initial.sql")),
    (2, include_str!("../../../../migrations/002_protocol.sql")),
    (3, include_str!("../../../../migrations/003_file_paths.sql")),
    (4, include_str!("../../../../migrations/004_job_template.sql")),
    (5, include_str!("../../../../migrations/005_job_error_class.sql")),
];

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

        apply_migrations(&conn).context("apply migrations")?;

        Ok(Arc::new(Self { conn: Arc::new(Mutex::new(conn)) }))
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Arc<Self>> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        apply_migrations(&conn)?;
        Ok(Arc::new(Self { conn: Arc::new(Mutex::new(conn)) }))
    }
}

fn apply_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _schema_version (version INTEGER PRIMARY KEY);",
    )?;

    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM _schema_version",
        [],
        |r| r.get(0),
    )?;

    for &(version, sql) in MIGRATIONS {
        if version > current {
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT OR IGNORE INTO _schema_version (version) VALUES (?1)",
                [version],
            )?;
        }
    }
    Ok(())
}
