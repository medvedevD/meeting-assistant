# Ralph-Loop Mission — Qt Migration (Meeting Assistant)

## Mission
Replace the Kotlin/Compose desktop UI with **Qt Quick/QML (Fusion)**, keeping
the **Rust core whole**. The Qt GUI runs the Rust core as a child **sidecar**
process over **loopback HTTP** (no FFI/cxx-qt). Deliver a polished v1 on
macOS/Linux/Windows with feature parity. All architecture is fixed (see the
embedded spec context below); do NOT re-litigate the 11 decisions.

## Execution rules
- Implement sections in dependency order (graph below). Do not start a section
  until its dependencies' acceptance criteria pass.
- Section 01 is FIRST (branch setup). Section 02 is the critical path.
  03/05/06 can run after their deps; 04 after 03; 07 after 02+03+05 (run the
  early macOS spike near phase-3 start); 08 grows alongside and gates ship.
- `ui-compose/` is FROZEN — read it only as a behavior/flow reference, never
  edit it, never copy its visual design.
- Verify every acceptance checkbox of a section before moving on.
- Do not push to any remote unless explicitly told.
- When ALL sections' acceptance criteria pass, emit exactly:
  `<promise>ALL-SECTIONS-COMPLETE</promise>`

## Dependency graph
```
Section 01 (branch setup) ── FIRST
[macOS packaging spike]   ── EARLY, before 03–05 go deep
Section 02 (sidecar)      ── critical path, after 01
  ├─> Section 03 (Qt skeleton) ─> Section 04 (QML screens)
  └─> Section 06 (crash-safe recovery)
Section 05 (macOS audio)  ── parallel, Rust-core only, after 01
Section 07 (packaging)    ── after 02+03+05
Section 08 (testing & CI) ── alongside 02–07; gates ship
```

## Fixed architecture (do not change)
Sidecar HTTP boundary (loopback 127.0.0.1, port 0, bearer token); core crash
must not crash GUI; macOS = Homebrew Cask `no_quarantine`, no paid Apple Dev ID
for now (macOS-only constraint); macOS system-audio via ScreenCaptureKit in v1;
crash-safe = startup recovery-pass (crash-friendly format is fast-follow); UI =
Qt Quick/QML; Fusion only (Basic/Material/Universal forbidden); sidecar
robustness (handshake/reaping/version) mandatory; version contract = protocol
range, hard-fail only on breaking; hard cutover (Compose frozen, behavior
reference only); plain C++ Qt + separate Rust binary (NO cxx-qt). Fast-follow
(NOT v1): crash-friendly format, UDS/named-pipe, native QML styles, auto-update,
Apple Developer ID, the IntelliJ design-spec workstream.

---

# Embedded sections

<!-- The full, self-contained content of each section follows. Implement in
order. Each section also exists as a file under sections/. -->

## === SECTION 01 — branch-setup ===
Delete the throwaway `proto/jewel-look-feel` (local-only, never pushed; its
verdict is captured in project memory + this plan — consciously reverses the
Section-06 "keep it" decision per owner instruction 2026-05-18). Steps:
(1) salvage `ui-compose/PROTOTYPE.md` if a repo copy must persist (content
already in project memory); (2) branch off the production-Compose base:
`git checkout feat/compose-desktop-rewrite && git checkout -b feat/qt-migration`
(confirm base at execution); ensure `.claude/plans/qt-migration/` is present on
it; (3) from `feat/qt-migration`: `git branch -D proto/jewel-look-feel`;
(4) all later work on `feat/qt-migration`, `ui-compose/` frozen there.
**Accept:** new branch off correct base with `ui-compose/` + planning dir
present; prototype branch gone locally; nothing pushed.

## === SECTION 02 — sidecar-server ===
Harden the existing `rust/crates/app/src/cli.rs Serve` (already binds
`127.0.0.1:port`, wires the full adapter graph via `container`, spawns the
worker; `meeting-api` has `AppState{transcriber,meeting_repo,job_repo,llm,
templates,audio_capture,recordings_dir}` + 7 routes, no auth yet) into
`rust/crates/app/src/bin/meeting-server.rs`. Add: bind `127.0.0.1:0` + read back
port; print ONE stdout handshake line first
`{"ready":true,"port","token":"<hex64>","protocol","min_protocol","build"}` then
flush; logger to **stderr only** (stdout reserved for handshake); random
256-bit bearer token, axum middleware requiring `Authorization: Bearer` on every
`/api/*` (401 else); hard-assert loopback IP; `GET /health` (no auth) 200 when
ready; `GET /version` (no auth) build+protocol+min_protocol;
`PROTOCOL_VERSION` defined once in `rust/crates/api/src/lib.rs` (C++ const
generated from it / CI-checked, see §03/§08); orphan reaping = inherited pipe
EOF on POSIX (preferred) or `--parent-pid`+`kill(pid,0)` poll, Windows Job
Object kill-on-close; reuse existing `try_acquire_singleton()` flock (distinct
exit code on contention); SIGTERM/SIGINT/CTRL → `WorkerHandle.stop_graceful`
then exit 0. **Accept:** handshake first on stdout, logs on stderr; loopback
only; `/api/*` 401 w/o token & 200 w/; `/health`+`/version` ok; parent death →
graceful exit ≤~2 s; 2nd instance → singleton code; SIGTERM → clean exit 0.

## === SECTION 03 — qt-skeleton ===
New top-level `qt-app/` (CMake, `Qt6 6.7 Quick Network`, `qt_add_executable`+
`qt_add_qml_module`). `main.cpp`: after `QGuiApplication`, before engine,
`QQuickStyle::setStyle("Fusion")` + compiled `qtquickcontrols2.conf`
`[Controls] Style=Fusion`; do NOT ship Material/Universal plugins. **No
cxx-qt/qt-build-utils.** `SidecarManager` (C++ QObject): locate sibling
`meeting-server` via `QCoreApplication::applicationDirPath()`, `QProcess::start`
with inherited pipe/`--parent-pid` (+ Windows Job Object kill-on-close), parse
first stdout line, poll `/health` (~15×200 ms) before ready,
`terminate()`→wait→`kill()`→`waitForFinished()` on quit, respawn-on-crash with
bounded budget + "core restarting" state. Version gate (Q9): compare compiled
`kClientProtocol` ∈ `[min_protocol,protocol]` → else blocking
"incompatible, update" dialog; build-only diff proceeds; `kClientProtocol`
**generated from Rust `PROTOCOL_VERSION` at build time**. `ApiClient`
(QNetworkAccessManager/QRestAccessManager): bearer header, base
`http://127.0.0.1:<port>`, JSON via QJsonDocument, signals to QML; `JobPoller`
QTimer on `/api/v1/jobs/:id`. Add top-level `run-qt.sh` (cargo build
meeting-server + cmake build qt-app + copy sidecar next to GUI). **Accept:**
launch spawns sidecar, handshake+health, Fusion verified; external kill →
restart; quit leaves no orphan (3 OSes, Windows via Job Object); version
mismatch → dialog; `kClientProtocol` generated; `run-qt.sh` runs the stack.

## === SECTION 04 — qml-screens ===
Port screen behavior/flows from frozen `ui-compose/` (behavior ONLY, never
visual design). (1) Inventory screens: MeetingList, MeetingDetail/Protocol,
NewRecording, GenerateProtocol, Settings, Diagnostics — data, state machine
(populated/empty/loading/error), nav, backing routes among the 7.
(2) **ViewModel audit** — `MeetingListVM`/`RecordingVM`/`ProtocolGenerateVM`
logic lives in Kotlin `shared/commonMain`, NOT the Rust API; map each piece →
move into core/API (preferred) or reimplement in Qt client; produce the mapping
table BEFORE writing screens. (3) Implement screens in QML/Fusion via
ApiClient: list→`GET /meetings`; record→`POST /recordings`+`/stop`;
transcribe→`POST /jobs`+JobPoller; detail→`POST /protocols` + **markdown view
(first-class; old Compose md lib does NOT port; baseline Qt `TextEdit`
`MarkdownText`, evaluate on real protocols incl. tables/code, document
limits)**; settings; diagnostics. (4) `StackView` nav. (5) No bespoke control
restyling. **Accept:** VM mapping table complete (nothing dropped); every flow
works via sidecar API only; 4 list data-states; full
record→transcribe→protocol round-trip; markdown renders headings/lists/tables/
code; Fusion only.

## === SECTION 05 — macos-audio ===
Rust-core, parallel to UI. Add `screencapturekit` crate (doom-fish v2.1.x) as
`[target.'cfg(target_os="macos")'.dependencies]` in
`rust/crates/adapters/Cargo.toml`. Replace the macOS `record_system` stub in
`rust/crates/adapters/src/audio/cpal_capture.rs` with `SCStream` **audio-only**
(`capturesAudio=true`, 48 kHz/2ch/f32 → same `hound` WAV path). Keep
stream-output object alive; **NO real screen output** — use large
`minimumFrameInterval`, never request/retain screen frames (document in code).
Mixed: SCStream(system)+cpal(mic) are independent clocks — do a 60-min drift
spike BEFORE copying the Linux mic+parec→ffmpeg_mix shape; either align/resample
or document a bounded-drift limitation. macOS floor 13.0; Info.plist
`NSScreenCaptureUsageDescription` (wired by §07). TCC re-prompt UX: v1 is
ad-hoc-signed so the grant resets each update — detect "not authorized", show a
clear in-app message + deep link to Privacy&Security→Screen Recording (first-run
== post-update). **Accept:** macOS 13+ System & Mixed yield valid WAVs ==
Linux/Win (Mixed w/ documented clock handling); no screen frames ever; denied
permission → guided message no crash; drift spike recorded.

## === SECTION 06 — crash-safe-recovery ===
Runs in the `meeting-server` boot path (§02). Codebase fact:
`cpal_capture.rs` streams to disk via `hound::WavWriter`+`BufWriter`, WAV only
finalized on clean stop. On startup, scan `meetings_dir` for
`<YYYY-MM-DD_HH-MM_uuid8>/recording.wav`; orphan if no `meetings` row for its
`audio_path` OR header unfinalized. **WAV reconstruction — do NOT assume 44-byte
header** (hound float WAV has `fmt `+`fact` chunks): parse RIFF chunk list from
offset 12 walking `ckID`/`ckSize` to the `data` chunk → `data_off`; payload =
`file_size - data_off` truncated to whole frame (`channels*4` for f32); patch
`data` ckSize + RIFF ckSize (`file_size-8`). Verify parser vs a real
codebase-produced hound f32 file (unit test: record→truncate→reconstruct→assert
playable+frame count). DB reconcile: recreate `meetings` row from slug if
missing. Idempotent. **Accept:** kill mid-recording → next start finalizes WAV;
recording appears in `/meetings` & transcribable; both orphan kinds; double-run
no-op; parser unit-tested vs real WAV.

## === SECTION 07 — packaging ===
Two-binary bundle (Qt GUI + `meeting-server`). **EARLY macOS spike near
phase-3 start** (before §03–05 deep): stub GUI+sidecar packaged, deep-signed
ad-hoc, cask-installed, helper launched, Screen-Recording TCC triggered, on
clean macOS 13 AND 15. macOS: `.app`, helper in `Contents/MacOS/`, deep-sign
(ad-hoc/self-signed v1; Apple Silicon needs ≥ad-hoc), Info.plist
`NSScreenCaptureUsageDescription`, `macdeployqt` (does NOT copy helper — CMake
install), Homebrew Cask `no_quarantine` + self-hosted tap fallback; **tracked
risk: Homebrew unsigned-cask sunset ~Sept 2026**, exit = buy Apple Dev ID +
notarize (fast-follow, dated). Linux: AppImage (`linuxdeploy` + qt plugin),
helper in AppDir, $0. Windows: `windeployqt` + Inno/WiX/NSIS, helper beside
`.exe`, unsigned → one-click SmartScreen. Cross: GUI finds sidecar via own exe
dir; both binaries version-pinned together (protocol check is the runtime net);
Qt LGPLv3 → dynamic link + source/written offer in all 3. **Accept:** early
spike passed (or blockers triaged); each OS clean-install launches & GUI starts
sidecar; version-pinned; Homebrew sunset documented w/ date+exit; LGPLv3 met.

## === SECTION 08 — testing-ci ===
Grows alongside 02–07, gates ship. (1) Sidecar contract tests (Rust integration
vs `meeting-server`): handshake valid JSON & first on stdout / logs stderr;
`/api/*` 401-no-token 200-with; loopback-only; `/version` + protocol-range
unit (in/below/above); parent-death via pipe/Job-Object → child exits in budget;
singleton 2nd-instance distinct code. (2) Recovery tests (§06): record→truncate→
recover→valid WAV+frame count+row; idempotent; both orphan kinds; RIFF parser
vs real hound f32 (not 44-byte). (3) Qt smoke (`QT_QPA_PLATFORM=offscreen`):
boot, handshake+health vs real server, meetings fetch round-trip,
version-mismatch dialog path. (4) CI matrix macOS+Linux+Windows: build
`meeting-server` + `qt-app` + run the above; CI check `PROTOCOL_VERSION` ==
C++ `kClientProtocol`. **Accept:** all three test suites pass; CI green on 3
OSes building both binaries; protocol-version equality enforced in CI.

---

When every section's acceptance criteria are verified:
`<promise>ALL-SECTIONS-COMPLETE</promise>`
