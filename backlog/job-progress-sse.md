# Job Progress via Server-Sent Events

## Context

[`JobPoller`](../qt-app/src/JobPoller.h) polls `GET /api/v1/jobs/:id` at a
fixed interval. Idle UI wakes the sidecar repeatedly; active UI sees stale
progress between polls. The `LiveProgress` DashMap already holds the
fine-grained data — the GUI just cannot subscribe to it.

## Goal

Stream job progress from the sidecar to the GUI in near real time, with the
existing polling path retained as a fallback (still useful for `/health` and
backwards compatibility).

## Sketch

- Add `GET /api/v1/jobs/:id/events` returning `text/event-stream`. Push every
  `JobProgress` update from the live table plus a terminal event when the row
  becomes done/failed.
- QML side: introduce a small `EventSource`-style C++ client (Qt has no
  builtin), or reuse `QNetworkAccessManager` with chunked-read. `JobPoller`
  becomes a fallback when the stream errors out.
- Depends on [[live-progress-unification]] so the writer/reader cannot drift.

## Expected Outcome

Progress bars update smoothly; idle CPU drops because the GUI no longer polls
when no jobs are active; polling still works on environments where SSE fails.
