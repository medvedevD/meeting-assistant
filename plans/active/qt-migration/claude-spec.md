# Qt Migration — Synthesized Specification

Synthesis of: the initial spec (`qt-migration-spec.md`), research findings
(`claude-research.md`), and the interview (`claude-interview.md`). This is the
authoritative requirements document for the implementation plan.

---

## 1. What we are building

Replace the Meeting Assistant desktop UI (Kotlin/Compose, `ui-compose`) with a
**Qt Quick / QML** app (Fusion style), keeping the existing **Rust core whole**.
The Qt GUI runs the Rust core as a **child sidecar process** and talks to it
over **loopback HTTP**. Target: a polished v1 that ships on **macOS / Linux /
Windows** with feature parity.

The Rust core is sound; the only unsolved problem of the old stack was UI look &
feel, which is a UI-layer problem. C++ rewrite of the core is rejected.

## 2. Fixed architectural constraints (locked, not re-litigated)

The 11 grill-session decisions from project memory hold verbatim:

1. Sidecar HTTP boundary (loopback `127.0.0.1`, port 0, bearer token); UDS/
   named-pipe is fast-follow plan B. NOT cxx-qt/C-ABI. Rust core unchanged.
2. Core crash must not crash the GUI.
3. macOS signing: Homebrew Cask `no_quarantine`, no paid Apple Developer ID for
   now (macOS-only constraint; Linux/Windows unaffected).
4. macOS system-audio (ScreenCaptureKit) in v1; feature parity on 3 OSes.
5. Crash-safe recording: v1 = startup recovery-pass (WAV-header rebuild);
   crash-friendly format = fast-follow.
6. UI = Qt Quick / QML.
7. Style = Fusion in v1; Basic/Material/Universal forbidden; native styles =
   fast-follow.
8. Sidecar robustness mandatory v1: startup handshake, orphan reaping, version
   check; single-instance via existing `SINGLETON_LOCK_FILE`.
9. Version contract = protocol-version range, hard-fail only on breaking
   mismatch (not build-version equality).
10. Hard cutover: Compose frozen (not deleted), reference for behavior/flows
    ONLY, never visual design.
11. Visual north-star = Fusion + IntelliJ/JetBrains layout; **design is a
    SEPARATE later workstream, not part of this plan**.

## 3. Interview resolutions (override/refine where noted)

- **Q3×Q4 tension resolved → accept the macOS TCC re-prompt.** macOS v1 is
  ad-hoc/self-signed; ScreenCaptureKit's Screen-Recording grant is re-requested
  after each update. The Homebrew unsigned-cask sunset (~Sept 2026) is a
  **tracked, dated risk**; Apple Developer ID is the documented eventual exit; a
  self-hosted tap is the fallback channel.
- **Critical path = sidecar hardening first** (a loopback `axum::serve` already
  exists in `app/src/cli.rs Serve` — harden, don't build from scratch).
- **Explicit fast-follow (NOT v1):** crash-friendly recording format; UDS/named-
  pipe transport; native macOS/Windows QML styles; auto-update.
- **Design-spec is out of scope** for this plan (separate workstream). QML
  sections enforce Fusion + behavior-port from frozen Compose; final visual
  design deferred.

## 4. Verified codebase baseline (what already exists)

- **`app/src/cli.rs Serve { port }`**: already wires the full adapter graph via
  a `container` and runs `axum::serve` bound to `127.0.0.1:port`, after spawning
  the worker. Sidecar = harden this (port 0 + handshake, `/health`, `/version`,
  token, reaping), not greenfield.
- **`meeting_api`**: `AppState{transcriber,meeting_repo,job_repo,llm,templates,
  audio_capture,recordings_dir}` + 7 routes (transcribe, jobs submit/status,
  protocols, recordings start/stop, meetings list). No auth/middleware yet.
- **`ffi/app_core.rs`**: full adapter wiring reference (Whisper/SQLite/Anthropic/
  Cpal/templates/file-store), `Worker` + `WorkerHandle.stop_graceful`,
  `SINGLETON_LOCK_FILE` flock at `$XDG_DATA/meeting-assistant/*.lock`. DB =
  rusqlite WAL + 3 migrations.
- **`adapters/audio/cpal_capture.rs`**: mic via cpal; Linux system via `parec`;
  Windows via WASAPI; **macOS `record_system` = unimplemented stub**. WAV via
  `hound` f32, `.finalize()` only on clean stop. No orphan-recording recovery.
- **Recording path**: `meetings_dir/<YYYY-MM-DD_HH-MM_uuid8>/recording.wav`;
  `meetings` table has `audio_path`; worker recovers stuck *jobs* but not
  *recordings*.
- **Workspace**: core←adapters←{api←app, ffi}. Tokio `1.*` full, axum `0.7`.
  Sidecar binary host = the `app` crate.

## 5. Web-research constraints baked into the plan

- **macOS audio**: `screencapturekit` crate (doom-fish, v2.1.x, maintained),
  macOS 13.0 floor, `NSScreenCaptureUsageDescription` in Info.plist, "Screen
  Recording" TCC framing accepted for v1. Footguns: keep stream-output objects
  alive; add a screen output or large `minimumFrameInterval` or audio drops;
  mic vs system on independent clocks.
- **Qt**: target **Qt 6.7+** (QRestAccessManager). C++ `QNetworkAccessManager`
  wrapper exposed to QML (bearer via `setRawHeader`, JSON via `QJsonDocument`,
  signals to QML, `QTimer` job polling). `QProcess` for the sidecar; parse port
  from `readyReadStandardOutput`; `/health` readiness gate;
  `terminate()`→`kill()`+`waitForFinished()`; **Windows parent-death = Job
  Objects**, POSIX = parent-PID poll. Fusion via `QQuickStyle::setStyle` +
  compiled `qtquickcontrols2.conf`; do not ship Material/Universal plugins.
  Plain C++ Qt + separate `cargo build` — **no cxx-qt/qt-build-utils**. Qt6
  LGPLv3 requires dynamic linking + source/written-offer.
- **Packaging**: macOS — `.app` with sidecar deep-signed in `Contents/MacOS/`,
  Homebrew Cask `no_quarantine` (+ self-hosted tap fallback), ad-hoc sign for
  Apple Silicon. Linux — AppImage (linuxdeploy + qt plugin) bundling Qt6/QML +
  sidecar; $0. Windows — `windeployqt` + Inno/WiX/NSIS installer; unsigned →
  one-click SmartScreen. GUI locates sibling sidecar via its own exe dir.

## 6. v1 acceptance (definition of done)

- Qt/QML app (Fusion enforced) reproduces the Compose app's screen behavior &
  flows; talks only to the sidecar over loopback HTTP with bearer token.
- Sidecar: port 0 + stdout port handshake, `/health`, `/version` (build +
  protocol + min-protocol), protocol-range check with explicit user-facing
  hard-fail on breaking mismatch, orphan reaping (POSIX PID poll + Windows Job
  Object), single-instance reuse.
- Core crash → GUI survives, shows a clear "core restarting" state, restarts the
  child; in-progress recording is recoverable.
- macOS system/mixed audio works via ScreenCaptureKit (parity with Linux/
  Windows); first-run + post-update Screen-Recording permission flow is handled
  gracefully in-app.
- Crash-safe recording: on core startup, orphaned in-progress recordings are
  found and their WAV headers reconstructed from on-disk data length; the
  recording is then visible/usable.
- Ships on all 3 OSes via the documented packaging; Compose `ui-compose` frozen,
  untouched, present as behavior reference.

## 7. Explicitly OUT of v1 (fast-follow backlog)

Crash-friendly recording format; UDS/named-pipe transport; native macOS/Windows
QML styles; auto-update (Sparkle/WinSparkle/AppImageUpdate); Apple Developer ID
purchase + notarization (tracked against the ~Sept-2026 Homebrew sunset); the
IntelliJ design-spec + visual-iteration workstream.
