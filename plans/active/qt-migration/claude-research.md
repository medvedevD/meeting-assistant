# Qt Migration — Research Findings

> Compiled from 3 research subagents (codebase / macOS-audio / Qt-sidecar) +
> owner-synthesized packaging section (the packaging subagent hit a usage limit;
> this section is reconstructed from the prior grill-session analysis + crumbs
> returned by the macOS-audio agent). Date: 2026-05-18.

---

## ⚠️ CRITICAL CROSS-CUTTING FINDING — re-examine before planning

Two research facts **directly challenge locked decisions Q3 + Q4** and must be
surfaced to the owner before the plan is finalized:

1. **Homebrew is deprecating unsigned / un-notarized casks by ~September 2026.**
   The "Homebrew Cask `no_quarantine`, no paid Apple Developer ID" path (Q3) has
   a shelf life. `no_quarantine` removes the quarantine bit, but Homebrew policy
   is moving to require signed+notarized artifacts; a self-hosted tap is the
   fallback but is lower-trust and still hits Gatekeeper for un-notarized apps.

2. **Ad-hoc signing breaks macOS TCC permissions on every rebuild.** macOS ties
   Screen-Recording / System-Audio TCC grants to the code-signing identity hash.
   With ad-hoc signing (`codesign -s -`) that hash changes every build, so the
   user must re-grant the Screen-Recording permission repeatedly. Q4 (macOS
   system-audio via ScreenCaptureKit, in v1) **requires a stable signing
   identity** to be usable in practice — which is exactly what Q3 deferred.

**Implication:** Q3 (no paid Apple Developer ID) and Q4 (ScreenCaptureKit audio
in v1) are in tension on macOS. Options: (a) accept that macOS system-audio
requires the user to re-grant permission after each app update until a stable
Developer ID is purchased; (b) bring the Developer ID purchase forward for macOS
specifically (the project memory already flags it as eventual/expected); (c)
ship macOS v1 with a stable *self-signed* cert that at least keeps TCC stable
within a release line (does NOT satisfy Gatekeeper, but Homebrew `no_quarantine`
covers launch). This is an interview topic, not a re-litigation of Qt-vs-Compose.

---

## 1. Codebase facts (verified — Rust core)

### 1.1 Adapter graph (`rust/crates/ffi/src/app_core.rs:185-264`)

`AppCore` constructs the full adapter graph. Concrete adapters + constructors:

| Adapter | Constructor | Inputs |
|---|---|---|
| `LazyWhisperTranscriber` | `::new(model_path, prefs)` | model_path, `TranscriberPrefs{language,beam_size,n_threads}` |
| `SqliteMeetingRepo` | `(Arc::clone(&db))` | shared `Db` |
| `SqliteJobRepo` | `(Arc::clone(&db))` | shared `Db` |
| `AnthropicProvider` | `::new(api_key)` | api_key (config→settings.json→`ANTHROPIC_API_KEY`→"") |
| `FileTemplateLoader` | `::new(&prompts_dir)` | prompts_dir |
| `CpalAudioCapture` | `::new()` | none |
| `FsMeetingFileStore` | unit struct | none |
| `JsonSettingsStore` | `::open_default()` | settings.json |

Path resolution priority: explicit config → `settings.json` → env var → XDG
default. Keys: `MEETING_ASSISTANT_MODEL` (→ `xdg_data/meeting-assistant/models/
ggml-medium.bin`), `MEETING_ASSISTANT_DB` (→ `xdg_data/.../rust-index.db`),
`MEETING_ASSISTANT_MEETINGS_DIR` (→ `xdg_documents/meeting-assistant`),
`MEETING_ASSISTANT_PROMPTS`.

DB: `Db::open(path)` → creates parent dir, opens rusqlite `Connection`, sets
`journal_mode=WAL` + `foreign_keys=ON`, applies 3 embedded migrations
(initial schema → +protocol_text → +transcript_path/protocol_path).
Connection is `Arc<Mutex<Connection>>`.

### 1.2 Worker + singleton (`app_core.rs`)

- `start_worker(core)` → `tokio::oneshot` shutdown channel, `Worker::new(job_repo,
  meeting_repo, transcriber, file_store)`, `tokio::spawn(worker.run(shutdown_rx))`.
  `WorkerHandle` exposes `stop_graceful(timeout_ms)` / `stop()` / `is_finished()`.
  Worker loop polls `claim_pending()` every 2 s; on startup calls
  `recover_running_jobs(now)` (resets stuck `running` jobs → `pending`).
- `SINGLETON_LOCK_FILE: OnceLock<File>`; `try_acquire_singleton()` flock-exclusive
  on `$XDG_DATA/meeting-assistant/meeting-assistant.lock` (advisory, OS-released
  on exit even after `kill -9`).

### 1.3 ⭐ A runnable server ALREADY EXISTS (spec assumption corrected)

The spec said "no `axum::serve` anywhere". **Research found `app/src/cli.rs`
already has a `Serve { port }` command** (~lines 190-209):

```rust
Command::Serve { port } => {
    let (_worker, _shutdown) = container.spawn_worker();
    let addr = SocketAddr::from(([127,0,0,1], port));   // already loopback-only
    let listener = TcpListener::bind(addr).await?;
    let state = meeting_api::AppState { /* full graph from container */ };
    axum::serve(listener, meeting_api::create_router(state)).await?;
}
```

So the sidecar is **not greenfield** — it is: (1) make `port` default to `0`
(ephemeral) and print the OS-chosen port on stdout; (2) add `/health`,
`/version`, bearer-token middleware, orphan-reaping, strict-loopback assertion;
(3) reuse `container` wiring (mirrors `app_core.rs`). This materially shrinks
the sidecar-binary section.

### 1.4 API surface (`api/src/router.rs` + `routes/`)

`AppState { transcriber, meeting_repo, job_repo, llm, templates, audio_capture,
recordings_dir }`. **No auth/middleware currently.** 7 routes:

| Method | Route | Req DTO | Resp DTO |
|---|---|---|---|
| POST | `/api/v1/transcribe` | `{path, meeting_id?}` | `{text,language,segments[]}` |
| POST | `/api/v1/jobs` | `{audio_path,name}` | `JobResponse` |
| GET | `/api/v1/jobs/:id` | — | `JobResponse` |
| POST | `/api/v1/protocols` | `{transcript,template_name,meeting_name}` | `{markdown}` |
| POST | `/api/v1/recordings` | `{name,source,echo_cancel}` | `MeetingResponse` |
| POST | `/api/v1/recordings/:id/stop` | — | `MeetingResponse` |
| GET | `/api/v1/meetings` | — | `MeetingItem[]` |

`JobResponse{id,meeting_id,kind,status,attempts,last_error,created_at,updated_at}`.
Status codes already differentiated (201/404/409/500).

### 1.5 Workspace (`rust/Cargo.toml`)

Members: core, adapters, api, app, ffi, uniffi-bindgen. Deps: core ← adapters ←
{api ← app, ffi}. Tokio `1.*` full; axum `0.7` (api + app only). Existing bins:
`meeting-assistant` (app), `uniffi-bindgen`. **Recommended host for the sidecar:
the `app` crate** (already depends on api, already has `Serve`) — add a focused
`meeting-server` bin or harden the `Serve` subcommand. Avoids a new crate.

### 1.6 Audio (`adapters/src/audio/cpal_capture.rs`, 540 ln)

`CpalAudioCapture{ sessions: Mutex<HashMap<String,Session>> }`; `Session{stop_tx:
SyncSender<()>, thread: JoinHandle}`. Per-source threads: `record_single` (mic,
cpal), `record_parec` (Linux system, spawns `parec --format=float32le
--channels=2 --rate=48000`), `record_mixed` (Linux: mic+parec→`ffmpeg_mix`;
Windows: WASAPI output). **macOS `record_system` = stub returning
`Err("system audio capture is not yet supported on this platform")`**
(lines ~228-231). WAV via `hound::WavSpec{channels:2|auto, sample_rate:48000|
auto, bits_per_sample:32, SampleFormat::Float}`; `.finalize()` only on clean
stop via `Arc::try_unwrap()`.

### 1.7 Recording lifecycle / orphan recovery

`start_recording`: UUID `meeting.id` → slug `YYYY-MM-DD_HH-MM_<uuid8>` →
`meetings_dir/<slug>/recording.wav`; `capture.start_session` THEN
`meeting_repo.save`. `stop_recording`: `stop_session` THEN `find_by_id`.
**No orphan-recording recovery exists.** Crash between `start_session` and
`save` (or before stop) → `.wav` on disk with unpatched header, no DB row.
Recovery hints: slug encodes timestamp + uuid8; no sidecar metadata file;
`meetings` table has `audio_path`. Worker already recovers *jobs*, not
*recordings*.

---

## 2. macOS system-audio from Rust (web research)

- **ScreenCaptureKit** `SCStreamConfiguration.capturesAudio=true` does audio-only
  capture; **macOS 13.0** floor. Triggers the **Screen Recording** TCC prompt
  (no audio-only prompt; framing is misleading for a meetings app). Needs
  `NSScreenCaptureUsageDescription` in Info.plist or the app is killed without a
  prompt. Sequoia (15): 30-day re-confirmation, persistent orange indicator.
  Footgun: must add a screen output or set large `minimumFrameInterval` or audio
  frames drop; must keep stream-output objects alive.
- **Core Audio process taps** (`AudioHardwareCreateProcessTap`/`CATapDescription`,
  macOS 14.2, stable 14.4+): uses the "System Audio Recording" TCC setting (better
  framing), per-process. **But: no maintained high-level Rust binding**, poorly
  documented, production reliability issues (attenuation, zeroed buffers). Not
  recommended yet.
- **Rust binding choice: `screencapturekit` crate (doom-fish, v2.1.x, actively
  maintained Feb 2026)** — idiomatic, 23 examples, `CMSampleBuffer`→f32 helpers.
  Alternatives: `objc2-screen-capture-kit` (low-level, maintained),
  `cidre`/`coreaudio-sys` (raw). `ruhear` is a useful cross-platform reference
  (Win=CPAL, Linux=Pulse, mac=SCK).
- **Distribution coupling (see CRITICAL finding):** ad-hoc signing changes the
  identity hash every build → TCC re-prompts. Stable signing identity strongly
  recommended for a usable Screen-Recording grant. Homebrew deprecating unsigned
  casks ~Sept 2026.
- **Recommendation:** standardize on `screencapturekit` crate, macOS 13.0 floor,
  `NSScreenCaptureUsageDescription`, accept "Screen Recording" framing for v1.

## 3. Qt Quick ↔ Rust sidecar (web research)

- **HTTP client:** prefer a **C++ wrapper over `QNetworkAccessManager`** exposed
  to QML (bearer header via `setRawHeader`, JSON via `QJsonDocument`, signals to
  QML). Qt 6.7+ adds `QRestAccessManager`/`QRestReply` (cleaner error class
  split) — target Qt 6.7+. Pure-QML `XMLHttpRequest` only for trivial cases.
  Job polling: C++ class owning a `QTimer`, emits `statusChanged` to QML, bounded
  backoff + `/health` readiness gate.
- **Process mgmt:** `QProcess::start(full_path_to_sidecar)`; parse port from
  `readyReadStandardOutput`; poll `/health` (~10×200 ms) before first call;
  `terminate()`→`kill()`+`waitForFinished()` on exit. Parent-death: POSIX poll
  parent PID; **Windows: Job Objects** to tie sidecar lifetime to parent.
  macOS bundle: sidecar in `Contents/MacOS/` (or `Resources/`), locate via
  `QCoreApplication::applicationDirPath()`; `macdeployqt` does NOT bundle the
  helper — copy via CMake install.
- **Fusion enforcement:** `QQuickStyle::setStyle("Fusion")` in `main()` after
  `QGuiApplication`, before engine load (overrides everything) + a compiled-in
  `:/qtquickcontrols2.conf` `[Controls] Style=Fusion` fallback; do not ship the
  Material/Universal style plugins. Fusion auto-tracks system light/dark on
  6.5+; HCI outlines in 6.10+.
- **Build:** plain C++ Qt app (CMake `qt_add_executable`+`qt_add_qml_module`,
  Qt6 Quick+Network) + **separate** `cargo build` for the sidecar; **do NOT use
  cxx-qt / qt-build-utils** (no FFI here). Bundle the Rust binary via CMake
  `install()` / cpack. `qt_generate_deploy_qml_app_script` handles Qt runtime.
- **Licensing:** Qt6 LGPLv3 is fine for a closed app **if dynamically linked** +
  provide Qt source or written offer + allow user re-linking (relevant to all 3
  packaging formats). Static linking would force source release.

## 4. Packaging ×3 OS (owner-synthesized; subagent unavailable)

- **macOS:** Homebrew Cask with `no_quarantine` removes the quarantine bit so the
  un-notarized `.app` launches without the Gatekeeper wall **for now** — but see
  the CRITICAL finding (Homebrew deprecating unsigned casks ~Sept 2026; ad-hoc
  signing breaks TCC). Apple Silicon requires at least ad-hoc signing to run at
  all; nested helper binary (the Rust sidecar) must be **deep-signed**
  (`codesign --deep` or sign helper then app) and live in `Contents/MacOS/`.
  Self-hosted tap is the fallback if not eligible for homebrew-cask. Sequoia
  removed the right-click→Open bypass (Privacy&Security "Open Anyway" only).
- **Linux:** AppImage (linuxdeploy + qt plugin bundles Qt6+QML runtime; Rust
  helper bundled beside the AppRun-launched GUI) — $0, no gate. `.deb`/Flatpak
  optional later. System-audio already works via `parec` (PulseAudio).
- **Windows:** Inno Setup/WiX/NSIS installer + `windeployqt` for the Qt runtime;
  Rust helper beside the `.exe`. No per-dev cert required to *run*; unsigned →
  one-click SmartScreen "More info → Run anyway" (reputation builds over
  downloads; optional OV cert later removes it). System-audio works via WASAPI.
- **Common:** GUI locates sibling sidecar via its own exe dir; version-pin both
  binaries in one bundle (feeds the Q9 protocol-version check — build skew is
  caught at runtime, not assumed away). Auto-update (Sparkle/WinSparkle/
  AppImageUpdate) must replace **both** binaries atomically.

---

## 5. Net effect on the locked decisions

- **Q1 sidecar:** confirmed easy — a loopback `axum::serve` already exists in
  `app/cli.rs Serve`. Sidecar section = harden, not build from zero.
- **Q3 + Q4 tension (NEW):** must be resolved in the interview — ScreenCaptureKit
  needs stable signing; Homebrew is sunsetting unsigned casks. Likely pulls the
  macOS Developer ID purchase earlier than "later".
- **Q5 orphan recovery:** confirmed absent; slug carries timestamp+uuid8, DB has
  `audio_path` — recovery is a filesystem scan + WAV-header rebuild + DB
  reconciliation.
- **Q6/Q7 Qt:** plain C++ Qt + separate Rust binary (no cxx-qt) confirmed
  simplest; Fusion enforcement mechanism is concrete; target **Qt 6.7+**.
- **Q8/Q9 lifecycle:** Windows parent-death needs **Job Objects** (not just PID
  polling) — a concrete cross-platform detail for the plan.
