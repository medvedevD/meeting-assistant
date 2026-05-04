-- Migration 002: extend jobs table with full RQ + progress tracking columns

ALTER TABLE jobs ADD COLUMN max_attempts INTEGER NOT NULL DEFAULT 3;
ALTER TABLE jobs ADD COLUMN progress_stage TEXT;
ALTER TABLE jobs ADD COLUMN progress_value REAL;
ALTER TABLE jobs ADD COLUMN progress_eta_sec REAL;
ALTER TABLE jobs ADD COLUMN params_json TEXT;
ALTER TABLE jobs ADD COLUMN error_message TEXT;
ALTER TABLE jobs ADD COLUMN error_kind TEXT;
ALTER TABLE jobs ADD COLUMN rq_job_id TEXT;
ALTER TABLE jobs ADD COLUMN enqueued_at TEXT;
ALTER TABLE jobs ADD COLUMN last_heartbeat_at TEXT;

CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status, enqueued_at);
CREATE INDEX IF NOT EXISTS idx_jobs_meeting ON jobs(meeting_slug, status);
