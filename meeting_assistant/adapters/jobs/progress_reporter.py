"""
ProgressReporter — used inside RQ workers to write progress to the DB
and publish events to Redis pub/sub for SSE delivery.

Designed to be created inside a worker process (not the web process).
"""
import json
import logging
import os
import sqlite3
from datetime import datetime
from pathlib import Path

_log = logging.getLogger(__name__)

_DEFAULT_DB = Path.home() / ".local" / "share" / "meeting-assistant" / "index.db"
_CHANNEL_PREFIX = "meeting:"
_CANCEL_KEY_PREFIX = "cancel:"


class ProgressReporter:
    def __init__(
        self,
        job_id: str,
        slug: str,
        db_path: Path = _DEFAULT_DB,
        redis_url: str | None = None,
    ):
        self._job_id = job_id
        self._slug = slug
        self._db_path = db_path
        self._redis_url = redis_url or os.environ.get("REDIS_URL", "redis://127.0.0.1:6379")
        self._redis = None  # lazy-init
        self._cached_rq_job_id: str | None = None  # cached after mark_running()

    # ── Redis ─────────────────────────────────────────────────────────────────

    def _get_redis(self):
        if self._redis is None:
            import redis
            self._redis = redis.from_url(self._redis_url)
        return self._redis

    def _publish(self, payload: dict) -> None:
        try:
            r = self._get_redis()
            r.publish(
                f"{_CHANNEL_PREFIX}{self._slug}:events",
                json.dumps(payload, ensure_ascii=False),
            )
        except Exception:
            pass  # never crash the worker because of pub/sub failure

    # ── DB ────────────────────────────────────────────────────────────────────

    def _connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(str(self._db_path))
        conn.execute("PRAGMA journal_mode=WAL")
        return conn

    # ── public API ────────────────────────────────────────────────────────────

    def mark_running(self) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET status='running', started_at=?, last_heartbeat_at=?,
                       progress_stage=NULL, progress_value=NULL
                   WHERE id=?""",
                (now, now, self._job_id),
            )
            # Warm the rq_job_id cache so should_cancel() never hits the DB.
            row = conn.execute(
                "SELECT rq_job_id FROM jobs WHERE id=?", (self._job_id,)
            ).fetchone()
            self._cached_rq_job_id = row[0] if row else None
        self._publish({"type": "status", "status": "running"})

    def update(
        self,
        stage: str,
        value: float | None,
        eta_sec: float | None = None,
        message: str = "",
    ) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET progress_stage=?, progress_value=?, progress_eta_sec=?,
                       last_heartbeat_at=?
                   WHERE id=?""",
                (stage, value, eta_sec, now, self._job_id),
            )
        self._publish({
            "type": "progress",
            "stage": stage,
            "value": value,
            "eta_sec": eta_sec,
            "message": message,
        })

    def heartbeat(self) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                "UPDATE jobs SET last_heartbeat_at=? WHERE id=?", (now, self._job_id)
            )

    def should_cancel(self) -> bool:
        rq_job_id = self._cached_rq_job_id
        if not rq_job_id:
            return False
        try:
            return bool(self._get_redis().get(f"{_CANCEL_KEY_PREFIX}{rq_job_id}"))
        except Exception:
            return False

    def mark_succeeded(self) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET status='succeeded', finished_at=?, progress_value=1.0
                   WHERE id=?""",
                (now, self._job_id),
            )
        self._publish({"type": "job_done", "status": "succeeded"})

    def mark_succeeded_no_publish(self) -> None:
        """Mark succeeded in DB without publishing job_done (used mid-pipeline)."""
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET status='succeeded', finished_at=?, progress_value=1.0
                   WHERE id=?""",
                (now, self._job_id),
            )

    def mark_failed(self, error_message: str, error_kind: str = "unknown") -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                """UPDATE jobs
                   SET status='failed', finished_at=?,
                       error_message=?, error_kind=?
                   WHERE id=?""",
                (now, error_message, error_kind, self._job_id),
            )
        self._publish({
            "type": "error",
            "error_kind": error_kind,
            "message": error_message,
        })

    def mark_cancelled(self) -> None:
        now = datetime.utcnow().isoformat()
        with self._connect() as conn:
            conn.execute(
                "UPDATE jobs SET status='cancelled', finished_at=? WHERE id=?",
                (now, self._job_id),
            )
        self._publish({"type": "cancelled"})
