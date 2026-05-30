-- Backend-owned transcribe->protocol chain. When a transcription job carries
-- then_protocol=1, the worker enqueues a protocol job on success. Persisting the
-- intent here (instead of holding it in client memory) keeps the chain alive
-- across app restarts and navigation, so the protocol step is never skipped.
ALTER TABLE jobs ADD COLUMN then_protocol INTEGER NOT NULL DEFAULT 0;
