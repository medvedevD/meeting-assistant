# Section 02 — Sidecar server binary + lifecycle/protocol contract

## Background
The Qt GUI talks to the Rust core over **loopback HTTP**; the core runs as a
child sidecar process (no FFI/cxx-qt). The core crash must not crash the GUI.
**Verified baseline:** `rust/crates/app/src/cli.rs` already has a
`Serve { port }` command that wires the full adapter graph via a `container`
and runs `axum::serve` bound to `127.0.0.1:port` after spawning the worker. The
`meeting-api` crate exposes `AppState{transcriber,meeting_repo,job_repo,llm,
templates,audio_capture,recordings_dir}` + 7 routes (transcribe; jobs
submit/status; protocols; recordings start/stop; meetings list) with **no auth
yet**. This section hardens that into a real sidecar; it is not greenfield.

## Requirements
A `meeting-server` process that: binds loopback-only on an OS-chosen port;
emits a single machine-readable handshake line on stdout; authenticates every
API route with a bearer token; serves `/health` and `/version`; exits cleanly
when its parent GUI dies; enforces single-instance; shuts down gracefully.

## Dependencies
- Requires: section-01 (branch).
- Blocks: section-03 (Qt client needs the contract), section-06 (recovery runs
  in this boot path), section-07 (packaging bundles this binary).
- Critical path — do first after branch setup.

## Implementation details
- **Host:** add `rust/crates/app/src/bin/meeting-server.rs` reusing the existing
  `container` wiring (mirror of `ffi/app_core.rs` adapter graph). Keep the old
  `Serve` subcommand or delegate it to the same code.
- **Ephemeral port + handshake:** bind `TcpListener` to `127.0.0.1:0`; read back
  `local_addr()`. Before serving, print exactly one line to **stdout**:
  `{"ready":true,"port":<n>,"token":"<hex64>","protocol":<int>,"min_protocol":<int>,"build":"<semver>"}\n`
  then flush. This MUST be the first bytes on stdout.
- **stdout discipline:** configure the logger (tracing/env_logger) to **stderr
  only** before anything else; stdout is reserved for the single handshake line.
- **Bearer token:** generate a random 256-bit hex token at startup; an axum
  middleware layer requires `Authorization: Bearer <token>` on every `/api/*`
  route (401 otherwise). Token travels only via the handshake (never argv,
  never logged).
- **Strict loopback:** hard-assert the bound IP is `127.0.0.1`; refuse to start
  otherwise.
- **`GET /health`** (no auth): 200 `{"status":"ok"}` once worker + adapters are
  ready.
- **`GET /version`** (no auth): `{"build","protocol","min_protocol"}`.
  `PROTOCOL_VERSION` is defined ONCE in Rust (`rust/crates/api/src/lib.rs`);
  the C++ client constant is generated from it at build time (see section-03) or
  guarded by a CI equality check (section-08). `protocol` bumps only on breaking
  IPC changes; additive route/field changes do not.
- **Orphan reaping:** preferred POSIX mechanism — the GUI passes an inherited
  pipe read-end; a watchdog task `read`s it and exits on EOF (parent died;
  race-free). Also accept `--parent-pid <pid>` + `kill(pid,0)` poll (~1 s) as a
  fallback. Windows — the GUI assigns the child to a Job Object with
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (preferred) or poll `OpenProcess`. On
  parent death: `WorkerHandle.stop_graceful(timeout)` then exit.
- **Single-instance:** reuse the existing `try_acquire_singleton()` flock
  (`SINGLETON_LOCK_FILE` at `$XDG_DATA/meeting-assistant/*.lock`). On contention
  exit with a distinct exit code so the GUI can surface "already running".
- **Graceful shutdown:** on SIGTERM/SIGINT (Unix) / CTRL-close (Windows):
  `WorkerHandle.stop_graceful`, finish in-flight requests, exit 0.

## Acceptance criteria
- [ ] `meeting-server` prints a valid JSON handshake as the first stdout bytes;
      logs go to stderr only.
- [ ] Binds `127.0.0.1` only; refuses non-loopback.
- [ ] All 7 `/api/*` routes return 401 without the bearer token, 200 with it.
- [ ] `/health` and `/version` respond without auth; `/version` carries
      build + protocol + min_protocol.
- [ ] Parent death (pipe EOF / Job Object / PID gone) → graceful worker stop
      then process exit within ~2 s.
- [ ] Second instance exits with the distinct singleton code; first survives.
- [ ] SIGTERM → graceful shutdown, exit 0.

## Files to create/modify
- Create `rust/crates/app/src/bin/meeting-server.rs`.
- Modify `rust/crates/api/src/router.rs` (auth middleware), add
  `rust/crates/api/src/routes/health.rs`, `routes/version.rs`.
- Add `PROTOCOL_VERSION` const in `rust/crates/api/src/lib.rs`.
- Modify `rust/crates/app/Cargo.toml` (new `[[bin]]`, deps: rand, etc.).
