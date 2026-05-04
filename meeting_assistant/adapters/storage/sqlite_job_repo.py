"""SQLite-backed job repository (CRUD for the jobs table)."""
import json
import sqlite3
import uuid
from datetime import datetime
from pathlib import Path

from ...domain.entities.job import Job

_DEFAULT_DB = Path.home() / ".local" / "share" / "meeting-assistant" / "index.db"


def _row_to_job(row: sqlite3.Row) -> Job:
    def _dt(v: str | None) -> datetime | None:
        if not v:
            return None
        try:
            return datetime.fromisoformat(v)
        except ValueError:
            return None

    return Job(
        id=row["id"],
        meeting_slug=row["meeting_slug"],
        kind=row["kind"],
        status=row["status"],
        attempt=row["attempt"] or 0,
        max_attempts=row["max_attempts"] or 3,
        progress_stage=row["progress_stage"],
        progress_value=row["progress_value"],
        progress_eta_sec=row["progress_eta_sec"],
        params_json=row["params_json"],
        error_message=row["error_message"],
        error_kind=row["error_kind"],
        rq_job_id=row["rq_job_id"],
        enqueued_at=_dt(row["enqueued_at"]),
        started_at=_dt(row["started_at"]),
        finished_at=_dt(row["finished_at"]),
        last_heartbeat_at=_dt(row["last_heartbeat_at"]),
    )


class SqliteJobRepo:
    def __init__(self, db_path: Path = _DEFAULT_DB):
        self._db_path = db_path

    def _connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(str(self._db_path))
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA journal_mode=WAL")
        return conn

    # ── write ─────────────────────────────────────────────────────────────────

    def create(
        self,
        meeting_slug: str,
        kind: str,
        params: dict | None = None,
        max_attempts: int = 3,
    ) -> Job:
        job_id = uuid.uuid4().hex
        params_json = json.dumps(params, ensure_ascii=False) if params else None
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """INSERT INTO jobs
                       (id, meeting_slug, kind, status, attempt, max_attempts,
                        params_json, enqueued_at, created_at)
                   VALUES (?, ?, ?, 'queued', 0, ?, ?, ?, ?)""",
                (job_id, meeting_slug, kind, max_attempts, params_json, now, now),
            )
        return Job(
            id=job_id,
            meeting_slug=meeting_slug,
            kind=kind,
            status="queued",
            attempt=0,
            max_attempts=max_attempts,
            params_json=params_json,
            enqueued_at=datetime.fromisoformat(now),
        )

    def set_rq_job_id(self, job_id: str, rq_job_id: str) -> None:
        with self._connect() as conn:
            conn.execute(
                "UPDATE jobs SET rq_job_id=? WHERE id=?", (rq_job_id, job_id)
            )

    def mark_running(self, job_id: str) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET status='running', started_at=?, last_heartbeat_at=?,
                       progress_stage=NULL, progress_value=NULL
                   WHERE id=?""",
                (now, now, job_id),
            )

    def update_progress(
        self,
        job_id: str,
        stage: str,
        value: float | None,
        eta_sec: float | None = None,
    ) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET progress_stage=?, progress_value=?, progress_eta_sec=?,
                       last_heartbeat_at=?
                   WHERE id=?""",
                (stage, value, eta_sec, now, job_id),
            )

    def heartbeat(self, job_id: str) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                "UPDATE jobs SET last_heartbeat_at=? WHERE id=?", (now, job_id)
            )

    def mark_succeeded(self, job_id: str) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET status='succeeded', finished_at=?, progress_value=1.0
                   WHERE id=?""",
                (now, job_id),
            )

    def mark_failed(
        self, job_id: str, error_message: str, error_kind: str = "unknown"
    ) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET status='failed', finished_at=?,
                       error_message=?, error_kind=?
                   WHERE id=?""",
                (now, error_message, error_kind, job_id),
            )

    def mark_cancelled(self, job_id: str) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                "UPDATE jobs SET status='cancelled', finished_at=? WHERE id=?",
                (now, job_id),
            )

    def requeue(self, job_id: str, new_rq_job_id: str) -> None:
        """Reset a failed/running job back to queued with a new RQ job ID."""
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET status='queued', rq_job_id=?, enqueued_at=?,
                       started_at=NULL, finished_at=NULL,
                       error_message=NULL, error_kind=NULL,
                       progress_value=NULL, progress_stage=NULL
                   WHERE id=?""",
                (new_rq_job_id, now, job_id),
            )

    def increment_attempt(self, job_id: str) -> None:
        with self._connect() as conn:
            conn.execute(
                "UPDATE jobs SET attempt = attempt + 1 WHERE id=?", (job_id,)
            )

    # ── read ──────────────────────────────────────────────────────────────────

    def get(self, job_id: str) -> Job | None:
        with self._connect() as conn:
            row = conn.execute(
                "SELECT * FROM jobs WHERE id=?", (job_id,)
            ).fetchone()
        return _row_to_job(row) if row else None

    def find_running(self) -> list[Job]:
        with self._connect() as conn:
            rows = conn.execute(
                "SELECT * FROM jobs WHERE status='running'"
            ).fetchall()
        return [_row_to_job(r) for r in rows]

    def find_by_slug(self, slug: str) -> list[Job]:
        with self._connect() as conn:
            rows = conn.execute(
                "SELECT * FROM jobs WHERE meeting_slug=? ORDER BY enqueued_at DESC",
                (slug,),
            ).fetchall()
        return [_row_to_job(r) for r in rows]

    def find_latest_for_slug(self, slug: str, kind: str) -> Job | None:
        with self._connect() as conn:
            row = conn.execute(
                """SELECT * FROM jobs WHERE meeting_slug=? AND kind=?
                   ORDER BY enqueued_at DESC LIMIT 1""",
                (slug, kind),
            ).fetchone()
        return _row_to_job(row) if row else None

    def find_active(self) -> list[Job]:
        """Jobs that are queued or running."""
        with self._connect() as conn:
            rows = conn.execute(
                "SELECT * FROM jobs WHERE status IN ('queued', 'running') ORDER BY enqueued_at ASC"
            ).fetchall()
        return [_row_to_job(r) for r in rows]

    def find_stale(self, stale_after_sec: int = 120) -> list[Job]:
        """Running jobs whose last heartbeat is older than stale_after_sec seconds."""
        cutoff = datetime.utcnow().isoformat()
        with self._connect() as conn:
            rows = conn.execute(
                """SELECT * FROM jobs
                   WHERE status = 'running'
                     AND last_heartbeat_at IS NOT NULL
                     AND datetime(last_heartbeat_at, ? || ' seconds') < datetime(?)""",
                (str(stale_after_sec), cutoff),
            ).fetchall()
        return [_row_to_job(r) for r in rows]
