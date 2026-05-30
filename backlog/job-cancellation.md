# Job Cancellation

## Context

There is no way to cancel a running job. A wedged transcription (huge audio,
slow LLM provider, runaway Whisper) can only be stopped by killing the
sidecar, which loses crash-safe recovery state and disrupts other jobs. The
UI has no "Cancel" affordance either.

## Goal

Add cooperative cancellation: a UI action can stop a specific job at the next
safe checkpoint without affecting other jobs or the sidecar process.

## Sketch

- Add `DELETE /api/v1/jobs/:id` returning `202 Accepted` if the job is
  pending/running, `204` if already terminal.
- Register a `CancellationToken` per job in the live table (one entry alongside
  `JobProgress`).
- Worker checks the token at every `set_stage` boundary and inside the Whisper
  loop between segments; on cancel, marks the job `failed` with
  `error_class=cancelled` and skips the `then_protocol` chain.
- QML adds a Cancel button on `MeetingDetailScreen` / active-jobs row; the
  store calls the new route via `ApiClient`.

## Expected Outcome

A user can cancel a stuck job from the UI; the sidecar keeps running; the DB
records the cancellation explicitly so re-processing and history are correct.
