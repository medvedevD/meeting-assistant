"""
SQLite read-model with FTS5 full-text search.
Source of truth remains the filesystem (meetings/<slug>/*.md).
This index is a derived projection — can be rebuilt at any time with reindex().
"""
import sqlite3
from dataclasses import dataclass
from pathlib import Path

from ...application.events import TranscriptReady, ProtocolGenerated, MeetingDeleted

_DEFAULT_DB = Path.home() / ".local" / "share" / "meeting-assistant" / "index.db"

_DDL = """
CREATE TABLE IF NOT EXISTS meetings (
    slug            TEXT PRIMARY KEY,
    title           TEXT NOT NULL DEFAULT '',
    started_at      TEXT,
    has_transcript  INTEGER NOT NULL DEFAULT 0,
    has_protocol    INTEGER NOT NULL DEFAULT 0,
    indexed_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE VIRTUAL TABLE IF NOT EXISTS meeting_search USING fts5(
    slug UNINDEXED,
    title,
    transcript_text,
    protocol_text
);
"""


@dataclass
class SearchResult:
    slug: str
    title: str
    snippet: str
    has_transcript: bool
    has_protocol: bool


class SqliteIndexAdapter:
    def __init__(self, db_path: Path = _DEFAULT_DB):
        db_path.parent.mkdir(parents=True, exist_ok=True)
        self._db_path = db_path
        self._init_db()

    # ── event handlers ──────────────────────────────────────────────────────

    def on_transcript_ready(self, event: TranscriptReady) -> None:
        slug = event.slug.value
        transcript_text = Path(event.transcript_path).read_text(encoding="utf-8") if event.transcript_path else ""
        title = slug.replace("_", " ")
        with self._connect() as conn:
            conn.execute(
                """INSERT INTO meetings(slug, title, has_transcript)
                   VALUES(?, ?, 1)
                   ON CONFLICT(slug) DO UPDATE SET
                       has_transcript=1, indexed_at=datetime('now')""",
                (slug, title),
            )
            conn.execute("DELETE FROM meeting_search WHERE slug=?", (slug,))
            conn.execute(
                "INSERT INTO meeting_search(slug, title, transcript_text, protocol_text) VALUES(?,?,?,?)",
                (slug, title, _strip_header(transcript_text), ""),
            )

    def on_protocol_generated(self, event: ProtocolGenerated) -> None:
        slug = event.slug.value
        protocol_text = Path(event.protocol_path).read_text(encoding="utf-8") if event.protocol_path else ""
        with self._connect() as conn:
            conn.execute(
                """INSERT INTO meetings(slug, has_protocol)
                   VALUES(?, 1)
                   ON CONFLICT(slug) DO UPDATE SET
                       has_protocol=1, indexed_at=datetime('now')""",
                (slug,),
            )
            # Merge new protocol text into existing FTS row
            row = conn.execute(
                "SELECT title, transcript_text FROM meeting_search WHERE slug=?", (slug,)
            ).fetchone()
            conn.execute("DELETE FROM meeting_search WHERE slug=?", (slug,))
            title = row[0] if row else slug.replace("_", " ")
            transcript = row[1] if row else ""
            conn.execute(
                "INSERT INTO meeting_search(slug, title, transcript_text, protocol_text) VALUES(?,?,?,?)",
                (slug, title, transcript, _strip_header(protocol_text)),
            )

    # ── search ──────────────────────────────────────────────────────────────

    def search(self, query: str, limit: int = 20) -> list[SearchResult]:
        if not query.strip():
            return []
        with self._connect() as conn:
            rows = conn.execute(
                """SELECT
                       ms.slug,
                       m.has_transcript,
                       m.has_protocol,
                       snippet(meeting_search, 2, '<b>', '</b>', '…', 20) AS snip
                   FROM meeting_search ms
                   LEFT JOIN meetings m ON m.slug = ms.slug
                   WHERE meeting_search MATCH ?
                   ORDER BY rank
                   LIMIT ?""",
                (query, limit),
            ).fetchall()
        return [
            SearchResult(
                slug=r[0],
                title=r[0].replace("_", " "),
                snippet=r[3] or "",
                has_transcript=bool(r[1]),
                has_protocol=bool(r[2]),
            )
            for r in rows
        ]

    # ── reindex ─────────────────────────────────────────────────────────────

    def reindex(self, meetings_dir: Path) -> int:
        """Rebuild index from filesystem. Returns number of meetings indexed."""
        if not meetings_dir.exists():
            return 0
        count = 0
        with self._connect() as conn:
            conn.execute("DELETE FROM meeting_search")
            conn.execute("DELETE FROM meetings")
            for folder in sorted(meetings_dir.iterdir()):
                if not folder.is_dir():
                    continue
                slug = folder.name
                title = slug.replace("_", " ")
                transcript_path = folder / "transcript.md"
                protocol_path = folder / "protocol.md"
                has_t = transcript_path.exists()
                has_p = protocol_path.exists()
                started_at = _date_from_slug(slug)
                conn.execute(
                    """INSERT OR REPLACE INTO meetings
                       (slug, title, started_at, has_transcript, has_protocol)
                       VALUES(?,?,?,?,?)""",
                    (slug, title, started_at, int(has_t), int(has_p)),
                )
                transcript_text = _strip_header(transcript_path.read_text(encoding="utf-8")) if has_t else ""
                protocol_text = _strip_header(protocol_path.read_text(encoding="utf-8")) if has_p else ""
                conn.execute(
                    "INSERT INTO meeting_search(slug, title, transcript_text, protocol_text) VALUES(?,?,?,?)",
                    (slug, title, transcript_text, protocol_text),
                )
                count += 1
        return count

    def on_meeting_deleted(self, event: MeetingDeleted) -> None:
        self.remove(event.slug.value)

    def remove(self, slug: str) -> None:
        with self._connect() as conn:
            conn.execute("DELETE FROM meetings WHERE slug=?", (slug,))
            conn.execute("DELETE FROM meeting_search WHERE slug=?", (slug,))

    # ── internal ────────────────────────────────────────────────────────────

    def _connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(str(self._db_path))
        conn.row_factory = sqlite3.Row
        return conn

    def _init_db(self) -> None:
        with self._connect() as conn:
            conn.executescript(_DDL)
            self._migrate(conn)

    def _migrate(self, conn: sqlite3.Connection) -> None:
        # Detect contentless FTS5 table (created with content='') — slug column returns NULL.
        # If so, drop and recreate so column data is actually stored.
        row = conn.execute(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='meeting_search'"
        ).fetchone()
        if row and "content=" in (row[0] or ""):
            conn.executescript(
                "DROP TABLE IF EXISTS meeting_search;"
                "CREATE VIRTUAL TABLE meeting_search USING fts5("
                "  slug UNINDEXED, title, transcript_text, protocol_text"
                ");"
            )


def _strip_header(text: str) -> str:
    """Remove leading markdown header lines (# ... and **...**) before indexing."""
    lines = text.splitlines()
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped and not stripped.startswith("#") and not stripped.startswith("**"):
            return "\n".join(lines[i:])
    return ""


def _date_from_slug(slug: str) -> str | None:
    try:
        from datetime import datetime
        datetime.strptime(slug[:10], "%Y-%m-%d")
        return slug[:10]
    except (ValueError, IndexError):
        return None
