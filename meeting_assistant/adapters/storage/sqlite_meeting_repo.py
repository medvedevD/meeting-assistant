"""
SQLite-backed meeting repository.

Wraps FilesystemMeetingRepository for all file I/O; adds SQLite state tracking
(status, meeting_events audit log). The same DB file as SqliteIndexAdapter.
"""
import json
import sqlite3
from datetime import datetime
from pathlib import Path
from typing import BinaryIO

from ...domain.entities.meeting import Meeting
from ...domain.ports.meeting_repository import IMeetingRepository
from ...domain.services.state_machine import MeetingStateMachine, InvalidTransitionError
from ...domain.value_objects.meeting_slug import MeetingSlug
from ...domain.value_objects.meeting_status import MeetingStatus
from .filesystem_meeting_repo import FilesystemMeetingRepository

_DEFAULT_DB = Path.home() / ".local" / "share" / "meeting-assistant" / "index.db"
_MIGRATIONS_DIR = Path(__file__).parent / "migrations"


def _load_migrations() -> list[tuple[int, str]]:
    """Read *.sql files from migrations/, return sorted list of (version, sql)."""
    result = []
    for path in sorted(_MIGRATIONS_DIR.glob("*.sql")):
        try:
            version = int(path.name.split("_")[0])
        except (ValueError, IndexError):
            continue
        result.append((version, path.read_text(encoding="utf-8")))
    return result


def _apply_migrations(conn: sqlite3.Connection) -> None:
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version ("
        "  version INTEGER PRIMARY KEY,"
        "  applied_at TEXT NOT NULL DEFAULT (datetime('now'))"
        ")"
    )
    conn.commit()
    applied: set[int] = {
        row[0] for row in conn.execute("SELECT version FROM schema_version").fetchall()
    }
    for version, sql in _load_migrations():
        if version in applied:
            continue
        for stmt in sql.split(";"):
            # Strip leading comment lines before checking emptiness
            lines = [l for l in stmt.splitlines() if not l.strip().startswith("--")]
            stmt = "\n".join(lines).strip()
            if not stmt:
                continue
            try:
                conn.execute(stmt)
            except sqlite3.OperationalError as e:
                msg = str(e)
                if "duplicate column" in msg or "already exists" in msg:
                    continue
                raise
        conn.execute(
            "INSERT OR IGNORE INTO schema_version(version) VALUES(?)", (version,)
        )
        conn.commit()


# ── repository ────────────────────────────────────────────────────────────────

class SqliteMeetingRepository(IMeetingRepository):
    """
    Decorator around FilesystemMeetingRepository.
    All file operations delegate to the inner repo.
    Status and audit-log are maintained in SQLite.
    """

    def __init__(self, inner: FilesystemMeetingRepository, db_path: Path = _DEFAULT_DB):
        self._inner = inner
        self._db_path = db_path
        db_path.parent.mkdir(parents=True, exist_ok=True)
        self._state_machine = MeetingStateMachine()
        with self._connect() as conn:
            _apply_migrations(conn)

    # ── IMeetingRepository delegation ─────────────────────────────────────────

    @property
    def meetings_dir(self) -> Path:
        return self._inner.meetings_dir

    def resolve_slug_for_audio(self, audio_path: Path, meeting_name: str) -> MeetingSlug:
        slug = self._inner.resolve_slug_for_audio(audio_path, meeting_name)
        self._ensure_record(slug)
        return slug

    def create_meeting_from_upload(
        self,
        title: str,
        upload_stream: BinaryIO,
        original_filename: str,
    ) -> Path:
        dest = self._inner.create_meeting_from_upload(title, upload_stream, original_filename)
        slug = MeetingSlug(dest.parent.name)
        audio_path_rel = str(dest)
        with self._connect() as conn:
            conn.execute(
                """INSERT INTO meetings(slug, title, status, audio_path, created_at)
                   VALUES(?, ?, ?, ?, datetime('now'))
                   ON CONFLICT(slug) DO UPDATE SET
                       audio_path=excluded.audio_path,
                       status=CASE WHEN status='CREATED' THEN 'AUDIO_READY' ELSE status END""",
                (slug.value, title or slug.value, MeetingStatus.AUDIO_READY.value, audio_path_rel),
            )
            self._log_event(conn, slug.value, "status_changed",
                            from_status=None, to_status=MeetingStatus.AUDIO_READY.value)
        return dest

    def get_meeting_dir(self, slug: MeetingSlug) -> Path:
        return self._inner.get_meeting_dir(slug)

    def save_transcript(self, slug: MeetingSlug, meeting_name: str, text: str, date_str: str) -> Path:
        path = self._inner.save_transcript(slug, meeting_name, text, date_str)
        with self._connect() as conn:
            row = conn.execute("SELECT status FROM meetings WHERE slug=?", (slug.value,)).fetchone()
            prev_status = row["status"] if row else None
            conn.execute(
                """INSERT INTO meetings(slug, title, status, transcript_path, has_transcript, created_at)
                   VALUES(?, ?, ?, ?, 1, datetime('now'))
                   ON CONFLICT(slug) DO UPDATE SET
                       has_transcript=1,
                       transcript_path=excluded.transcript_path,
                       status='TRANSCRIBED'""",
                (slug.value, meeting_name, MeetingStatus.TRANSCRIBED.value, str(path)),
            )
            self._log_event(conn, slug.value, "status_changed",
                            from_status=prev_status, to_status=MeetingStatus.TRANSCRIBED.value,
                            details={"transcript_path": str(path)})
        return path

    def load_transcript_text(self, slug: MeetingSlug) -> str:
        return self._inner.load_transcript_text(slug)

    def save_protocol(self, slug: MeetingSlug, meeting_name: str, content: str, date_str: str) -> Path:
        path = self._inner.save_protocol(slug, meeting_name, content, date_str)
        with self._connect() as conn:
            row = conn.execute("SELECT status FROM meetings WHERE slug=?", (slug.value,)).fetchone()
            prev_status = row["status"] if row else None
            conn.execute(
                """INSERT INTO meetings(slug, title, status, protocol_path, has_protocol, created_at)
                   VALUES(?, ?, ?, ?, 1, datetime('now'))
                   ON CONFLICT(slug) DO UPDATE SET
                       has_protocol=1,
                       protocol_path=excluded.protocol_path,
                       status='COMPLETED'""",
                (slug.value, meeting_name, MeetingStatus.COMPLETED.value, str(path)),
            )
            self._log_event(conn, slug.value, "status_changed",
                            from_status=prev_status, to_status=MeetingStatus.COMPLETED.value,
                            details={"protocol_path": str(path)})
        return path

    def list(self) -> list[Meeting]:
        """Read from SQLite. Falls back to filesystem for meetings not yet in DB."""
        with self._connect() as conn:
            rows = conn.execute(
                """SELECT slug, title, status, started_at, audio_path,
                          transcript_path, protocol_path,
                          transcription_model, protocol_model, template_name,
                          has_transcript, has_protocol, archived_at
                   FROM meetings
                   WHERE deleted_at IS NULL
                   ORDER BY COALESCE(started_at, indexed_at, created_at) DESC"""
            ).fetchall()

        if rows:
            return [self._row_to_meeting(r) for r in rows]

        # DB empty — return from filesystem (backfill hasn't run yet)
        return self._inner.list()

    def get(self, slug: MeetingSlug) -> Meeting | None:
        with self._connect() as conn:
            row = conn.execute(
                """SELECT slug, title, status, started_at, audio_path,
                          transcript_path, protocol_path,
                          transcription_model, protocol_model, template_name,
                          has_transcript, has_protocol, archived_at
                   FROM meetings WHERE slug=? AND deleted_at IS NULL""",
                (slug.value,),
            ).fetchone()
        if row is None:
            return None
        return self._row_to_meeting(row)

    def delete(self, slug: MeetingSlug) -> None:
        self._inner.delete(slug)
        with self._connect() as conn:
            conn.execute(
                "UPDATE meetings SET deleted_at=datetime('now') WHERE slug=?",
                (slug.value,),
            )
            self._log_event(conn, slug.value, "meeting_deleted")

    def has_transcript(self, slug: MeetingSlug) -> bool:
        with self._connect() as conn:
            row = conn.execute(
                "SELECT has_transcript FROM meetings WHERE slug=?", (slug.value,)
            ).fetchone()
        if row is not None:
            return bool(row[0])
        return (self._inner.get_meeting_dir(slug) / "transcript.md").exists()

    def has_protocol(self, slug: MeetingSlug) -> bool:
        with self._connect() as conn:
            row = conn.execute(
                "SELECT has_protocol FROM meetings WHERE slug=?", (slug.value,)
            ).fetchone()
        if row is not None:
            return bool(row[0])
        return (self._inner.get_meeting_dir(slug) / "protocol.md").exists()

    def update_status(self, slug: MeetingSlug, new_status: MeetingStatus) -> None:
        with self._connect() as conn:
            row = conn.execute(
                "SELECT status FROM meetings WHERE slug=?", (slug.value,)
            ).fetchone()

            if row is None:
                # Meeting not yet in DB — insert with target status
                conn.execute(
                    """INSERT OR IGNORE INTO meetings(slug, title, status, created_at)
                       VALUES(?, ?, ?, datetime('now'))""",
                    (slug.value, slug.value.replace("_", " "), new_status.value),
                )
                self._log_event(conn, slug.value, "status_changed",
                                from_status=None, to_status=new_status.value)
                return

            current = MeetingStatus(row[0])
            self._state_machine.validate(current, new_status)
            conn.execute(
                "UPDATE meetings SET status=? WHERE slug=?",
                (new_status.value, slug.value),
            )
            self._log_event(conn, slug.value, "status_changed",
                            from_status=current.value, to_status=new_status.value)

    def delete_transcript(self, slug: MeetingSlug) -> None:
        self._inner.delete_transcript(slug)
        has_recording = any(self._inner.get_meeting_dir(slug).glob("recording.*"))
        new_status = MeetingStatus.AUDIO_READY if has_recording else MeetingStatus.CREATED
        with self._connect() as conn:
            conn.execute(
                "UPDATE meetings SET has_transcript=0, transcript_path=NULL, status=?, has_protocol=0, protocol_path=NULL WHERE slug=?",
                (new_status.value, slug.value),
            )
            self._log_event(conn, slug.value, "transcript_deleted",
                            to_status=new_status.value)

    def delete_protocol(self, slug: MeetingSlug) -> None:
        self._inner.delete_protocol(slug)
        has_t = self._inner.has_transcript(slug)
        new_status = MeetingStatus.TRANSCRIBED if has_t else MeetingStatus.AUDIO_READY
        with self._connect() as conn:
            conn.execute(
                "UPDATE meetings SET has_protocol=0, protocol_path=NULL, status=? WHERE slug=?",
                (new_status.value, slug.value),
            )
            self._log_event(conn, slug.value, "protocol_deleted",
                            to_status=new_status.value)

    def delete_audio(self, slug: MeetingSlug) -> None:
        self._inner.delete_audio(slug)
        with self._connect() as conn:
            conn.execute(
                "UPDATE meetings SET audio_path=NULL WHERE slug=?",
                (slug.value,),
            )
            self._log_event(conn, slug.value, "audio_deleted")

    def update_processing_metadata(
        self,
        slug: MeetingSlug,
        *,
        transcription_model: str | None = None,
        protocol_model: str | None = None,
        template_name: str | None = None,
    ) -> None:
        """Persist per-meeting model/template info recorded at job completion."""
        sets = []
        vals = []
        if transcription_model is not None:
            sets.append("transcription_model=?")
            vals.append(transcription_model)
        if protocol_model is not None:
            sets.append("protocol_model=?")
            vals.append(protocol_model)
        if template_name is not None:
            sets.append("template_name=?")
            vals.append(template_name)
        if not sets:
            return
        vals.append(slug.value)
        with self._connect() as conn:
            conn.execute(
                f"UPDATE meetings SET {', '.join(sets)} WHERE slug=?",
                vals,
            )

    # ── backfill ──────────────────────────────────────────────────────────────

    def backfill(self) -> int:
        """
        Scan meetings_dir, upsert a DB record for every meeting folder.
        Infers status from file presence. Idempotent — safe to run multiple times.
        Returns number of records upserted.
        """
        meetings_dir = self._inner.meetings_dir
        if not meetings_dir.exists():
            return 0
        count = 0
        with self._connect() as conn:
            for folder in meetings_dir.iterdir():
                if not folder.is_dir():
                    continue
                try:
                    slug = MeetingSlug(folder.name)
                except ValueError:
                    continue
                has_recording = any(folder.glob("recording.*"))
                has_t = (folder / "transcript.md").exists()
                has_p = (folder / "protocol.md").exists()

                if has_p:
                    status = MeetingStatus.COMPLETED
                elif has_t:
                    status = MeetingStatus.TRANSCRIBED
                elif has_recording:
                    status = MeetingStatus.AUDIO_READY
                else:
                    status = MeetingStatus.CREATED

                title = folder.name.replace("_", " ")
                started_at = _date_from_slug(folder.name)
                transcript_path = str(folder / "transcript.md") if has_t else None
                protocol_path = str(folder / "protocol.md") if has_p else None

                # Upsert — only update status if not already set to a more "advanced" state
                existing = conn.execute(
                    "SELECT status FROM meetings WHERE slug=?", (slug.value,)
                ).fetchone()

                if existing is None:
                    conn.execute(
                        """INSERT INTO meetings(slug, title, status, started_at,
                               transcript_path, protocol_path,
                               has_transcript, has_protocol, created_at)
                           VALUES(?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))""",
                        (slug.value, title, status.value, started_at,
                         transcript_path, protocol_path, int(has_t), int(has_p)),
                    )
                    self._log_event(conn, slug.value, "backfill",
                                    from_status=None, to_status=status.value)
                else:
                    # Update file-presence columns; also advance status for idle states
                    # (never overwrite in-progress states like TRANSCRIBING/GENERATING)
                    conn.execute(
                        """UPDATE meetings SET
                               has_transcript=?, has_protocol=?,
                               transcript_path=COALESCE(?, transcript_path),
                               protocol_path=COALESCE(?, protocol_path),
                               status=CASE
                                   WHEN status IN ('CREATED','AUDIO_READY','TRANSCRIBED') THEN ?
                                   ELSE status
                               END
                           WHERE slug=?""",
                        (int(has_t), int(has_p), transcript_path, protocol_path,
                         status.value, slug.value),
                    )
                count += 1
        return count

    # ── internals ─────────────────────────────────────────────────────────────

    def _connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(str(self._db_path))
        conn.row_factory = sqlite3.Row
        return conn

    def _ensure_record(self, slug: MeetingSlug) -> None:
        with self._connect() as conn:
            conn.execute(
                """INSERT OR IGNORE INTO meetings(slug, title, status, created_at)
                   VALUES(?, ?, ?, datetime('now'))""",
                (slug.value, slug.value.replace("_", " "), MeetingStatus.CREATED.value),
            )

    @staticmethod
    def _log_event(
        conn: sqlite3.Connection,
        slug: str,
        event_type: str,
        *,
        from_status: str | None = None,
        to_status: str | None = None,
        details: dict | None = None,
    ) -> None:
        conn.execute(
            """INSERT INTO meeting_events(meeting_slug, event_type, from_status, to_status, details)
               VALUES(?, ?, ?, ?, ?)""",
            (slug, event_type, from_status, to_status,
             json.dumps(details, ensure_ascii=False) if details else None),
        )

    @staticmethod
    def _row_to_meeting(row: sqlite3.Row) -> Meeting:
        from datetime import datetime as dt
        started_raw = row["started_at"]
        started_at = None
        if started_raw:
            try:
                started_at = dt.strptime(started_raw[:10], "%Y-%m-%d")
            except ValueError:
                pass

        archived_raw = row["archived_at"]
        archived_at = None
        if archived_raw:
            try:
                archived_at = dt.fromisoformat(archived_raw)
            except ValueError:
                pass

        return Meeting(
            slug=MeetingSlug(row["slug"]),
            title=row["title"] or row["slug"].replace("_", " "),
            status=MeetingStatus(row["status"]) if row["status"] else MeetingStatus.CREATED,
            started_at=started_at,
            audio_path=Path(row["audio_path"]) if row["audio_path"] else None,
            transcript_path=Path(row["transcript_path"]) if row["transcript_path"] else None,
            protocol_path=Path(row["protocol_path"]) if row["protocol_path"] else None,
            transcription_model=row["transcription_model"],
            protocol_model=row["protocol_model"],
            template_name=row["template_name"],
            has_transcript=bool(row["has_transcript"]),
            has_protocol=bool(row["has_protocol"]),
            archived_at=archived_at,
        )


def _date_from_slug(slug: str) -> str | None:
    try:
        datetime.strptime(slug[:10], "%Y-%m-%d")
        return slug[:10]
    except (ValueError, IndexError):
        return None
