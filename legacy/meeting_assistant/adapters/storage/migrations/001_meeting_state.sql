-- Migration 001: add state-machine columns to meetings, create meeting_events and jobs tables

ALTER TABLE meetings ADD COLUMN status TEXT NOT NULL DEFAULT 'COMPLETED';
ALTER TABLE meetings ADD COLUMN audio_path TEXT;
ALTER TABLE meetings ADD COLUMN audio_size_bytes INTEGER;
ALTER TABLE meetings ADD COLUMN audio_format TEXT;
ALTER TABLE meetings ADD COLUMN transcript_path TEXT;
ALTER TABLE meetings ADD COLUMN protocol_path TEXT;
ALTER TABLE meetings ADD COLUMN template_name TEXT;
ALTER TABLE meetings ADD COLUMN transcription_model TEXT;
ALTER TABLE meetings ADD COLUMN protocol_model TEXT;
ALTER TABLE meetings ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE meetings ADD COLUMN archived_at TEXT;
ALTER TABLE meetings ADD COLUMN deleted_at TEXT;

CREATE TABLE IF NOT EXISTS meeting_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_slug TEXT NOT NULL,
    event_type   TEXT NOT NULL,
    from_status  TEXT,
    to_status    TEXT,
    details      TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS jobs (
    id           TEXT PRIMARY KEY,
    meeting_slug TEXT NOT NULL,
    kind         TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'queued',
    attempt      INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    started_at   TEXT,
    finished_at  TEXT,
    error        TEXT
);
