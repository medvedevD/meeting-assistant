mod job_repo;
mod meeting_repo;

pub use job_repo::SqliteJobRepo;
pub use meeting_repo::SqliteMeetingRepo;

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
struct Migration {
    version: i64,
    up: &'static str,
    down: Option<&'static str>,
}

// Applied in order; already-applied versions are skipped. The shipped 001-006
// migrations are the forward-only baseline. New migrations may add a matching
// `down` script so developers can roll back local schema experiments.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        up: include_str!("../../../../migrations/001_initial.sql"),
        down: None,
    },
    Migration {
        version: 2,
        up: include_str!("../../../../migrations/002_protocol.sql"),
        down: None,
    },
    Migration {
        version: 3,
        up: include_str!("../../../../migrations/003_file_paths.sql"),
        down: None,
    },
    Migration {
        version: 4,
        up: include_str!("../../../../migrations/004_job_template.sql"),
        down: None,
    },
    Migration {
        version: 5,
        up: include_str!("../../../../migrations/005_job_error_class.sql"),
        down: None,
    },
    Migration {
        version: 6,
        up: include_str!("../../../../migrations/006_job_then_protocol.sql"),
        down: None,
    },
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
        let conn =
            Connection::open(path).with_context(|| format!("open sqlite at {}", path.display()))?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        apply_migrations(&conn).context("apply migrations")?;

        Ok(Arc::new(Self {
            conn: Arc::new(Mutex::new(conn)),
        }))
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Arc<Self>> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        apply_migrations(&conn)?;
        Ok(Arc::new(Self {
            conn: Arc::new(Mutex::new(conn)),
        }))
    }
}

fn apply_migrations(conn: &Connection) -> Result<()> {
    apply_migrations_from(conn, MIGRATIONS)
}

fn apply_migrations_from(conn: &Connection, migrations: &[Migration]) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _schema_version (version INTEGER PRIMARY KEY);",
    )?;

    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM _schema_version",
        [],
        |r| r.get(0),
    )?;

    for migration in migrations {
        if migration.version > current {
            conn.execute_batch(migration.up)
                .with_context(|| format!("apply migration {}", migration.version))?;
            conn.execute(
                "INSERT OR IGNORE INTO _schema_version (version) VALUES (?1)",
                [migration.version],
            )?;
        }
    }
    Ok(())
}

/// Rolls back the latest applied migration in the database at `path`.
///
/// This is intentionally a narrow development hook for `meeting-server
/// --rollback-one`: normal application startup never calls it and continues to
/// apply `up` migrations only.
pub fn rollback_last_migration_at_path(path: &Path) -> Result<i64> {
    let conn =
        Connection::open(path).with_context(|| format!("open sqlite at {}", path.display()))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    rollback_last_migration(&conn, MIGRATIONS)
}

fn rollback_last_migration(conn: &Connection, migrations: &[Migration]) -> Result<i64> {
    ensure_schema_version_table(conn)?;

    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM _schema_version",
        [],
        |r| r.get(0),
    )?;
    if current == 0 {
        bail!("no applied migrations to roll back");
    }

    let Some(migration) = migrations.iter().find(|m| m.version == current) else {
        bail!("latest applied migration {current} is not known to this binary");
    };
    let Some(down) = migration.down else {
        bail!("migration {current} has no down migration");
    };

    conn.execute_batch(down)
        .with_context(|| format!("roll back migration {current}"))?;
    conn.execute(
        "DELETE FROM _schema_version WHERE version = ?1",
        [migration.version],
    )?;
    Ok(current)
}

fn ensure_schema_version_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _schema_version (version INTEGER PRIMARY KEY);",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            up: "CREATE TABLE things (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
            down: Some("DROP TABLE things;"),
        },
        Migration {
            version: 2,
            up: "ALTER TABLE things ADD COLUMN note TEXT;",
            down: Some(
                "CREATE TABLE things_new (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
                 INSERT INTO things_new (id, name) SELECT id, name FROM things;
                 DROP TABLE things;
                 ALTER TABLE things_new RENAME TO things;",
            ),
        },
    ];

    #[test]
    fn rollback_last_migration_runs_down_and_removes_schema_version() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations_from(&conn, TEST_MIGRATIONS).unwrap();

        let rolled_back = rollback_last_migration(&conn, TEST_MIGRATIONS).unwrap();

        assert_eq!(rolled_back, 2);
        assert_eq!(current_version(&conn), 1);
        assert_eq!(column_exists(&conn, "things", "note"), false);
        assert_eq!(column_exists(&conn, "things", "name"), true);
    }

    #[test]
    fn rollback_refuses_forward_only_migration() {
        let conn = Connection::open_in_memory().unwrap();
        let migrations = &[Migration {
            version: 1,
            up: "CREATE TABLE things (id INTEGER PRIMARY KEY);",
            down: None,
        }];
        apply_migrations_from(&conn, migrations).unwrap();

        let err = rollback_last_migration(&conn, migrations).unwrap_err();

        assert!(err.to_string().contains("has no down migration"));
        assert_eq!(current_version(&conn), 1);
    }

    #[test]
    fn rollback_refuses_unknown_latest_version() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema_version_table(&conn).unwrap();
        conn.execute("INSERT INTO _schema_version (version) VALUES (99)", [])
            .unwrap();

        let err = rollback_last_migration(&conn, TEST_MIGRATIONS).unwrap_err();

        assert!(err.to_string().contains("is not known to this binary"));
        assert_eq!(current_version(&conn), 99);
    }

    fn current_version(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _schema_version",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        let mut columns = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
        columns.any(|name| name.as_deref() == Ok(column))
    }
}
