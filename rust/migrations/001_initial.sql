CREATE TABLE IF NOT EXISTS meetings (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    audio_path TEXT NOT NULL,
    transcript_text TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id),
    kind TEXT NOT NULL DEFAULT 'transcribe',
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    retry_after INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_status_retry ON jobs(status, retry_after);
