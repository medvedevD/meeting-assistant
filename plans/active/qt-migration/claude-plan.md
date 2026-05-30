# Qt Migration — Implementation Plan

> Self-contained. An engineer/LLM with no prior context can implement from this
> document alone. Project: **Meeting Assistant** — a Rust-core desktop app for
> recording, transcribing, and generating meeting protocols, at
> `/Users/dmitrymedvedev/projects/pets/meeting-assistant`.

## 0. Context & goal (read first)

The app today = a Rust 6-crate workspace (core/adapters/api/app/ffi/uniffi-bindgen)
+ a Kotlin/Compose Desktop UI (`ui-compose/`). The Compose UI looks amateurish /
mobile even after desktop-ing attempts. **Decision: replace the UI with Qt Quick/
QML, keep the Rust core unchanged.** The Qt GUI spawns the Rust core as a child
**sidecar** process and talks to it over **loopback HTTP**. No FFI/cxx-qt. v1
ships on macOS/Linux/Windows with feature parity.

All 11 architectural decisions are fixed (see `claude-spec.md`). This plan is
**engineering only** — the visual design-spec is a separate later workstream.

**Critical path:** Section 1 (sidecar) first; it unblocks everything else.

**Verified baseline:** `app/src/cli.rs` already has a `Serve { port }` command
that wires the full adapter graph and runs `axum::serve` on `127.0.0.1:port`.
The sidecar work is **hardening this**, not greenfield.

---

## Section 0.1 — Branch setup (FIRST action, before any code)

**Owner instruction (2026-05-18):** delete the throwaway prototype branch and
implement this plan on a fresh branch.

> ⚠️ **This consciously REVERSES the Section-06 decision** ("leave
> `proto/jewel-look-feel` unmerged and local — not deleted"). The reversal is
> deliberate and safe: the prototype's only value was the Qt-vs-Compose
> verdict, which is now fully captured in project memory and in this plan. The
> branch is **local-only (never pushed)** so deletion needs no remote cleanup.

Steps:

1. **Salvage first.** Anything that exists ONLY on `proto/jewel-look-feel` and
   must survive: the filled verdict in `ui-compose/PROTOTYPE.md`. Its content is
   already mirrored in project memory; if a repo copy must persist, copy
   `PROTOTYPE.md` into `.claude/plans/qt-migration/` before deletion. The
   `.claude/plans/qt-migration/` artifacts are NOT on the prototype branch — see
   step 2.
2. **Create the implementation branch from the production-Compose base, NOT from
   the prototype.** `ui-compose/` (the Section 3 behavior reference) lives on
   `feat/compose-desktop-rewrite`. Branch off it (confirm the exact base at
   execution time): `git checkout feat/compose-desktop-rewrite && git checkout
   -b feat/qt-migration`. Ensure the `.claude/plans/qt-migration/` planning
   files are present on the new branch (copy from wherever they were authored).
3. **Delete the prototype branch:** `git branch -D proto/jewel-look-feel`
   (only after steps 1–2 are verified; cannot delete the checked-out branch, so
   this runs from `feat/qt-migration`). Do NOT push anything unless the owner
   explicitly asks.
4. All subsequent sections are implemented on `feat/qt-migration`. `ui-compose/`
   stays frozen there as the behavior reference (Q10).

**Acceptance:** `feat/qt-migration` exists off the correct base with `ui-compose/`
present and the planning dir intact; `proto/jewel-look-feel` no longer exists
locally; nothing was pushed.

---

## Section 1 — Sidecar server binary + lifecycle/protocol contract

**Why:** the integration boundary; critical path; blocks Sections 2 & 3.

**Where:** the `app` crate (already depends on `meeting-api`, already has
`Serve`). Add a dedicated `meeting-server` binary (`rust/crates/app/src/bin/
meeting-server.rs`) reusing the existing `container` wiring, OR harden the
existing `Serve` subcommand and document it as the sidecar entrypoint. Prefer a
dedicated bin for a clean contract.

**Implement:**

1. **Ephemeral port + stdout handshake.** Bind `TcpListener` to `127.0.0.1:0`
   (OS picks a free port). After `bind`, read back `listener.local_addr()` and
   print a single machine-readable line to **stdout** before serving, e.g.
   `{"ready":true,"port":<n>,"token":"<hex>","protocol":<int>,
   "min_protocol":<int>,"build":"<semver>"}\n`. Flush. The GUI parses this line.
   **(L2) stdout discipline:** the handshake line MUST be the first bytes on
   stdout. All logging goes to **stderr only**; stdout is reserved exclusively
   for the single handshake line. Configure the logger (tracing/env_logger) to
   stderr before anything else runs.
   **(M3) protocol single source of truth:** define `PROTOCOL_VERSION` once in
   Rust (`rust/crates/api/src/lib.rs`). Generate the C++ `kClientProtocol`
   constant from it at build time (a tiny codegen step writing a header), OR add
   a CI check asserting the two literals are equal. Never hand-maintain both.
2. **Bearer token.** Generate a random 256-bit token at startup (hex). Require
   `Authorization: Bearer <token>` on every `/api/*` route via an axum
   middleware layer. Return 401 without it. Token is delivered only via the
   stdout handshake (never logged, never on argv).
3. **Strict loopback assertion.** Hard-assert the bound IP is `127.0.0.1`
   (never `0.0.0.0`); refuse to start otherwise.
4. **`GET /health`** (no auth): returns 200 `{"status":"ok"}` once the worker +
   adapters are initialized. Used by the GUI readiness gate.
5. **`GET /version`** (no auth): `{"build":"<semver>","protocol":<int>,
   "min_protocol":<int>}`. `protocol` bumps ONLY on breaking IPC changes;
   additive route/field changes do not bump it. Define the current protocol
   integer as a `const` in a shared spot.
6. **Orphan reaping.** **(L1) Preferred POSIX mechanism: an inherited pipe.**
   The GUI creates a pipe and passes the read end to the child; the child
   `read`s it in a watchdog task — EOF means the parent died (race-free, no
   PID-reuse hazard). Fallback / also accept `--parent-pid <pid>` and poll
   `kill(pid,0)` every ~1 s if the pipe approach is unavailable. Windows →
   assign the child to a **Job Object** with
   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` set by the GUI side (preferred), or poll
   `OpenProcess`. On parent death: trigger `WorkerHandle.stop_graceful(timeout)`
   then exit.
7. **Single-instance.** Reuse the existing `try_acquire_singleton()` flock
   (`SINGLETON_LOCK_FILE` at `$XDG_DATA/meeting-assistant/*.lock`). On contention
   exit with a distinct code so the GUI can surface "already running".
8. **Graceful shutdown.** On `SIGTERM`/`SIGINT` (Unix) and CTRL-close (Windows):
   `WorkerHandle.stop_graceful`, finish in-flight requests, exit 0.

**Acceptance:**
- `meeting-server --parent-pid <pid>` prints the handshake JSON on stdout, binds
  loopback-only, serves the 7 existing routes behind bearer auth, answers
  `/health` and `/version`.
- Killing the parent PID makes the server exit within ~2 s after graceful worker
  stop.
- Second instance exits with the singleton code; first keeps running.

**Files:** `rust/crates/app/src/bin/meeting-server.rs` (new), small additions to
`rust/crates/api` for the auth middleware + `/health` + `/version`
(`rust/crates/api/src/router.rs`, new `routes/health.rs`, `routes/version.rs`),
a `PROTOCOL_VERSION` const (e.g. `rust/crates/api/src/lib.rs`).

---

## Section 2 — Qt/QML app skeleton: Fusion, HTTP client, sidecar process mgmt

**Why:** the GUI shell + the client half of the boundary. Depends on Section 1
(needs the handshake/contract). Blocks Section 3.

**Where:** new top-level `qt-app/` directory (sibling of `ui-compose/`). CMake +
Qt 6.7+ (Quick, Network). **No cxx-qt / qt-build-utils.**

**Implement:**

1. **Project skeleton.** `qt-app/CMakeLists.txt` with
   `find_package(Qt6 6.7 REQUIRED COMPONENTS Quick Network)`,
   `qt_add_executable`, `qt_add_qml_module`, `qt_standard_project_setup`. A
   `main.cpp` that constructs `QGuiApplication`, calls
   `QQuickStyle::setStyle("Fusion")` **before** loading the engine, then loads
   `qml/Main.qml`. Add a compiled-in `qtquickcontrols2.conf` (`[Controls]
   Style=Fusion`) as belt-and-suspenders. Do not link/ship Material/Universal
   style plugins.
2. **Sidecar manager (C++).** A `SidecarManager` QObject: locate the sibling
   `meeting-server` binary via `QCoreApplication::applicationDirPath()`
   (per-OS layout from Section 6), `QProcess::start(path, {"--parent-pid",
   QString::number(QCoreApplication::applicationPid())})`. Parse the first
   stdout line → `{port, token, protocol, min_protocol, build}`. Poll
   `GET /health` (bounded: ~15 attempts × 200 ms) before declaring ready. On
   app quit: `terminate()` → wait → `kill()` → `waitForFinished()`. On Windows,
   create a Job Object with kill-on-close and assign the child. On unexpected
   child exit while the app is running: show a "core restarting…" state,
   respawn (bounded restart budget), reset to ready.
3. **Version gate (Q9).** After handshake, compare the GUI's compiled
   `kClientProtocol` against the server's `[min_protocol, protocol]`. If
   `kClientProtocol` ∉ range → show an explicit blocking dialog
   ("Components are incompatible — please update") and do not proceed. If in
   range → proceed even when `build` strings differ. **(M3)** `kClientProtocol`
   is generated from the Rust `PROTOCOL_VERSION` at build time (see Section 1),
   not hand-written.
   **(L4) Build entrypoint:** add a top-level build script (successor to
   `run-compose.sh`, e.g. `run-qt.sh`) that does `cargo build` of
   `meeting-server` + the CMake build of `qt-app/` + copies the sidecar binary
   next to the GUI for local dev. State this is the canonical dev workflow.
4. **HTTP client (C++ → QML).** An `ApiClient` QObject wrapping
   `QNetworkAccessManager` (or `QRestAccessManager`, Qt 6.7+). Injects
   `Authorization: Bearer <token>` and targets `http://127.0.0.1:<port>`.
   JSON via `QJsonDocument`. Exposes async methods + signals
   (`requestSucceeded`/`requestFailed`) to QML. A `JobPoller` helper owns a
   `QTimer` to poll `GET /api/v1/jobs/:id` and emit `statusChanged` to QML.
   Register types with `qmlRegisterType` / context properties.

**Acceptance:**
- Launching `qt-app` spawns `meeting-server`, completes the handshake + health
  gate, and the QML root renders in Fusion (verified via `palette` inspection /
  visual).
- Killing `meeting-server` externally → GUI shows "core restarting", respawns,
  recovers. Quitting the GUI leaves no orphan `meeting-server` (verify on all 3
  OSes; Windows via Job Object).
- A simulated protocol-range mismatch shows the blocking dialog; a build-only
  difference does not.

**Files:** `qt-app/` (new): `CMakeLists.txt`, `src/main.cpp`,
`src/SidecarManager.{h,cpp}`, `src/ApiClient.{h,cpp}`, `src/JobPoller.{h,cpp}`,
`qml/Main.qml`, `resources.qrc`, `qtquickcontrols2.conf`.

---

## Section 3 — QML screens: behavior port from frozen Compose

**Why:** the actual UI. Depends on Section 2 (shell + ApiClient). Compose
`ui-compose/` is the **behavior/flow reference only** — never copy its visual
design.

**Implement:**

1. **Inventory.** From `ui-compose/` enumerate screens & flows (NOT styling):
   MeetingList, MeetingDetail/Protocol, NewRecording, GenerateProtocol,
   Settings, Diagnostics. For each, capture: what data it shows, the state
   machine (populated/empty/loading/error), navigation edges, and which sidecar
   route(s) back it (map to the 7 routes in Section 1).
1b. **(H3) ViewModel audit.** Flow/business logic currently lives in Kotlin
   `shared/commonMain` ViewModels (`MeetingListVM`, `RecordingVM`,
   `ProtocolGenerateVM`), NOT in the Rust API. Audit each VM; for every piece of
   logic decide: move it into the Rust core/API (**preferred** — keeps the Qt
   client thin) or reimplement it in the Qt client. Produce an explicit
   VM→destination mapping table before writing screens; this prevents silently
   dropping behavior.
2. **Implement each screen in QML** with Fusion controls, driven by `ApiClient`:
   - MeetingList → `GET /api/v1/meetings`, with empty/loading/error states.
   - NewRecording → `POST /api/v1/recordings` (name, source, echo_cancel) /
     `POST /api/v1/recordings/:id/stop`; source picker incl. system/mixed.
   - Detail/Protocol → meeting data + `POST /api/v1/protocols`.
     **(L3) Markdown rendering is a first-class deliverable, not an aside:** the
     protocol view is the app's core output; the old Compose Material markdown
     lib does NOT port. Decide the Qt path explicitly (Qt `TextEdit`
     `textFormat: MarkdownText` is the cheapest viable baseline; evaluate it
     against real generated protocols incl. tables/headings/code before
     committing to anything heavier). Has its own acceptance bullet below.
   - Transcription/jobs → `POST /api/v1/jobs` + `JobPoller` on
     `GET /api/v1/jobs/:id`.
   - Settings → settings persisted by the core; expose existing settings keys.
   - Diagnostics → log/health surface.
3. **Navigation** between screens (Qt Quick `StackView`/loader); state survives
   sidecar restart where reasonable.
4. **No bespoke control restyling** (Q7). Layout/density is plain Fusion +
   sane spacing until the later design-spec workstream refines it.

**Acceptance:**
- Every Compose flow has a working QML equivalent driven only through the
  sidecar API; all four data-states render for the list; a full
  record→transcribe→generate-protocol round-trip works on the real core.
- The VM→destination mapping table (step 1b) exists and every VM behavior is
  accounted for (moved to core/API or reimplemented), none dropped.
- **(L3)** The protocol markdown view correctly renders a real generated
  protocol including headings, lists, tables and code blocks; the chosen render
  path is documented with its limitations.

**Files:** `qt-app/qml/screens/*.qml`, view-model C++/QML glue as needed.

---

## Section 4 — macOS system-audio via ScreenCaptureKit (parallel workstream)

**Why:** Q4 parity — macOS `record_system` is an unimplemented stub. Depends
ONLY on the Rust core; runs in parallel with Sections 1–3.

**Implement:**

1. Add the **`screencapturekit` crate** (doom-fish, v2.1.x) as a macOS-only dep
   in `rust/crates/adapters/Cargo.toml` (`[target.'cfg(target_os="macos")'.
   dependencies]`).
2. Replace the macOS `record_system` stub in
   `rust/crates/adapters/src/audio/cpal_capture.rs` with an SCStream audio-only
   capture: `SCStreamConfiguration.capturesAudio=true`, 48 kHz/2ch f32, write to
   the same `hound` WAV path as the other backends. Footguns to handle: keep the
   stream-output object alive for the session lifetime.
   **(M2) Do NOT instantiate a real screen output.** Use the large
   `minimumFrameInterval` workaround so audio frames are not dropped; never
   request, retain, or process screen frames (privacy + resource + permission
   scope). Document this explicitly in code.
   **(M1) Mixed-audio clock alignment is a real problem, not a copy of Linux.**
   SCStream (system) and cpal (mic) run on independent clocks and drift over a
   long meeting. v1 must either timestamp-align/resample the two streams before
   mixing, or explicitly document a bounded-drift limitation. Do a focused spike
   to measure drift over a 60-min capture BEFORE relying on the Linux
   mic+parec→`ffmpeg_mix` shape; treat `record_mixed` on macOS as the riskiest
   sub-task.
3. macOS floor **13.0**. Add `NSScreenCaptureUsageDescription` to the bundle
   Info.plist (Section 6 wires it into packaging).
4. **TCC re-prompt UX (interview decision).** Because v1 is ad-hoc/self-signed,
   the Screen-Recording grant resets on each update. Detect "not authorized"
   from SCStream and surface a clear, actionable in-app message + a deep link to
   System Settings → Privacy & Security → Screen Recording. Handle first-run and
   post-update identically.

**Acceptance:** on macOS 13+, `CaptureSource::System` and `::Mixed` produce
valid WAVs equivalent to Linux/Windows; denied permission yields a clear guided
message, not a silent failure or crash.

**Files:** `rust/crates/adapters/src/audio/cpal_capture.rs` (macOS branch),
`rust/crates/adapters/Cargo.toml`, a macОS SCK helper module.

---

## Section 5 — Crash-safe recording recovery

**Why:** Q5 — companion to sidecar crash isolation; without it isolation is
cosmetic (GUI survives, recording is an unplayable WAV). Depends on Section 1
(runs in the sidecar's core startup path).

**Implement (v1 = recovery-pass only; crash-friendly format is fast-follow):**

1. On core startup (in the sidecar boot path), scan `meetings_dir` for
   `*/recording.wav` files. For each, determine if it is an **orphan**: file
   exists but (a) no `meetings` row references its `audio_path`, or (b) a row
   exists but the WAV header is unfinalized (data-chunk length 0 / inconsistent
   with file size).
2. **WAV-header reconstruction (H1 — do NOT assume a 44-byte header).** `hound`
   writes float WAV as `WAVE_FORMAT_IEEE_FLOAT`, which conventionally includes a
   `fmt ` (18+ byte) chunk and a `fact` chunk — the header is **not** 44 bytes.
   Algorithm: parse the RIFF chunk list from offset 12, walking `ckID`/`ckSize`
   until the `data` chunk; record its byte offset `data_off`. True payload
   length = `file_size - data_off`, truncated down to a whole sample frame
   (`channels * 4` bytes for f32). Patch the `data` chunk `ckSize` and the
   top-level RIFF `ckSize` (`= file_size - 8` after truncation) in place.
   **Verify the parser against a real `hound` f32 file produced by this
   codebase before implementing the recovery pass** (write a unit test that
   round-trips: record → truncate → reconstruct → assert playable + sample
   count).
3. **DB reconciliation:** if no `meetings` row, recreate one from the slug
   (`<YYYY-MM-DD_HH-MM_uuid8>` → timestamp + uuid8) so the recording is visible
   in the list and can be transcribed. Idempotent: safe to run every startup.

**Acceptance:** kill the sidecar mid-recording; on next start the WAV is
finalized and the meeting appears and is transcribable. Re-running startup twice
is a no-op.

**Files:** new `rust/crates/.../recovery.rs` (likely in `adapters` or a core
usecase), called from the sidecar boot sequence (Section 1).

---

## Section 6 — Packaging ×3 OS (two-binary bundle)

**Why:** ship. Depends on Sections 1 & 2 (both binaries exist); needs Section 4
for the macOS Info.plist key.

**Implement:**

1. **macOS.** Build `MeetingAssistant.app`; place `meeting-server` in
   `Contents/MacOS/` beside the GUI; **deep-sign** (sign helper then app, or
   `codesign --deep`) — ad-hoc/self-signed for v1 (Apple Silicon requires at
   least ad-hoc to run). Add `NSScreenCaptureUsageDescription` to Info.plist.
   Bundle Qt via `macdeployqt` (note: it does NOT copy the helper — copy via
   CMake install). Distribute via a **Homebrew Cask** with `no_quarantine`;
   prepare a **self-hosted tap** as the fallback. **Tracked risk:** Homebrew is
   deprecating unsigned casks ~Sept 2026 — the documented exit is buying an
   Apple Developer ID + notarization (fast-follow backlog item with a hard
   date).
2. **Linux.** AppImage via `linuxdeploy` + the Qt plugin (bundles Qt6 + QML
   runtime); `meeting-server` bundled beside the GUI inside the AppDir; `.desktop`
   integration. $0, no signing gate. (`.deb`/Flatpak optional later.)
3. **Windows.** `windeployqt` for the Qt runtime; installer via Inno Setup (or
   WiX/NSIS); `meeting-server.exe` beside the GUI `.exe`. No cert required to
   run; unsigned → one-click SmartScreen "More info → Run anyway" (documented;
   OV cert is fast-follow).
4. **Cross-cutting.** The GUI locates the sidecar via its own exe dir on every
   OS. Both binaries are built from the same revision and version-pinned
   together; the protocol-version check (Section 1/2) is the runtime safety net.
   **Qt LGPLv3 compliance:** dynamic-link Qt; include Qt source offer / written
   offer in all three artifacts.

**(M4) Early macOS packaging spike — do this BEFORE Sections 3–5 go deep.**
The macOS chain (ad-hoc sign + nested helper deep-sign + Homebrew
`no_quarantine` + ScreenCaptureKit TCC + Sequoia Gatekeeper) is the single most
fragile, most likely to dead-end area. Run a minimal end-to-end spike on a clean
macOS 13 AND macOS 15 machine: a stub GUI + stub sidecar, packaged, deep-signed
ad-hoc, installed via the cask, launching the helper, and successfully
triggering the Screen-Recording TCC grant. Findings feed Sections 4 & 6 before
large effort is sunk.

**Acceptance:** a clean machine on each OS can install and launch the bundle;
the GUI starts the sidecar and is fully functional; uninstall is clean; the
macOS Homebrew sunset risk is documented in the repo with its date and exit; the
early macOS spike has passed (or its blockers are recorded and triaged).

**Files:** `qt-app/CMakeLists.txt` install rules, `packaging/macos/*` (cask,
entitlements, Info.plist), `packaging/linux/*` (AppImage recipe),
`packaging/windows/*` (installer script).

---

## Section 7 — Testing & CI (cross-cutting, H2)

**Why:** the "polished shippable" goal demands the new high-risk surface (the
sidecar contract, the recovery pass) be tested, and that both binaries build on
all 3 OSes. This is not optional.

**Implement:**

1. **Sidecar contract tests** (Rust integration tests against `meeting-server`):
   handshake line is valid JSON & first-on-stdout; `/api/*` returns 401 without
   the bearer token and 200 with it; bind is loopback-only; `/version` present;
   protocol-range gate logic (unit-test the comparison). Parent-death: spawn
   with a pipe/Job-Object, drop the parent, assert the child exits within the
   budget. Singleton: second instance exits with the distinct code.
2. **Recovery-pass tests** (Section 5): record→truncate WAV mid-stream→run
   recovery→assert the file is a valid WAV with the expected frame count and the
   meeting row exists; idempotency (run twice = no-op); both orphan kinds
   (no DB row / unfinalized header).
3. **Qt client smoke**: a headless/offscreen (`QT_QPA_PLATFORM=offscreen`) test
   that boots the GUI, completes the handshake + health gate against a real
   `meeting-server`, and asserts a list fetch round-trips. Version-mismatch
   simulation asserts the blocking dialog path.
4. **CI matrix**: GitHub Actions (or equivalent) building **macOS + Linux +
   Windows**: `cargo build` of `meeting-server`, CMake build of `qt-app/`, run
   the Rust contract/recovery tests + the offscreen Qt smoke. A CI check
   asserting the Rust `PROTOCOL_VERSION` and the generated/declared C++
   `kClientProtocol` match (M3).

**Acceptance:** the CI matrix is green on all 3 OSes; contract, recovery, and Qt
smoke tests run in CI; the protocol-version equality check is enforced.

**Files:** `rust/crates/app/tests/sidecar_contract.rs`,
`rust/crates/.../recovery_tests.rs`, `qt-app/tests/`, `.github/workflows/*`.

---

## Appendix A — Explicitly OUT of v1 (fast-follow backlog)

1. Crash-friendly recording format (streaming, no trailing finalize) — replaces
   the recovery-pass.
2. UDS / named-pipe transport — plan B only if a concrete loopback problem
   surfaces.
3. Native macOS/Windows QML styles — Fusion-only in v1.
4. Auto-update (Sparkle / WinSparkle / AppImageUpdate) for the two-binary bundle.
5. **Apple Developer ID + notarization** — dated against the ~Sept-2026 Homebrew
   unsigned-cask sunset; removes the macOS TCC-re-prompt and Gatekeeper friction.
6. IntelliJ/JetBrains design-spec + owner-driven visual-iteration loop (separate
   workstream; this plan ships plain Fusion).

## Appendix B — Dependency graph & suggested order

```
Section 0.1 (branch setup) ── FIRST, before any code
[macOS packaging spike (M4)] ── do EARLY, before 3–5 go deep
Section 1 (sidecar)  ── critical path, first
   ├─> Section 2 (Qt shell + client)
   │      └─> Section 3 (QML screens)
   └─> Section 5 (crash-safe recovery; uses sidecar boot path)
Section 4 (macOS audio) ── parallel, Rust-core only, any time
Section 6 (packaging) ── after 1+2 exist; needs 4 for Info.plist
Section 7 (testing & CI) ── grows alongside 1–6; gates ship
```

Compose `ui-compose/` stays **frozen and present** as behavior reference for
Section 3; zero edits to it.
