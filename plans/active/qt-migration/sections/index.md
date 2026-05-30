<!-- SECTION_MANIFEST
section-01-branch-setup
section-02-sidecar-server
section-03-qt-skeleton
section-04-qml-screens
section-05-macos-audio
section-06-crash-safe-recovery
section-07-packaging
section-08-testing-ci
END_MANIFEST -->

# Qt Migration — Implementation Sections Index

Source: `claude-plan.md`. The Rust core stays whole; only the UI is replaced
(Qt Quick/QML, Fusion) and the core runs as a loopback-HTTP sidecar. All 11
architectural decisions are fixed (see `claude-spec.md`).

## Dependency Graph

| Section | Depends On | Blocks | Parallelizable |
|---|---|---|---|
| section-01-branch-setup | - | all | No (must be first) |
| section-02-sidecar-server | 01 | 03, 04, 06, 07 | No (critical path) |
| section-03-qt-skeleton | 02 | 04, 07 | No |
| section-04-qml-screens | 03 | - | No |
| section-05-macos-audio | 01 | 07 | Yes (Rust-core only, parallel to 02–04) |
| section-06-crash-safe-recovery | 02 | - | Yes (parallel to 03–04) |
| section-07-packaging | 02, 03, 05 | - | No (incl. EARLY macОS spike) |
| section-08-testing-ci | 02, 03, 05, 06 | ship | Grows alongside 02–07 |

## Execution Order

1. **section-01-branch-setup** (delete throwaway prototype branch, create
   `feat/qt-migration` off the Compose base — FIRST, before any code).
2. **section-02-sidecar-server** (critical path; harden the existing
   `app Serve` into a real sidecar).
3. After 02: **section-03-qt-skeleton** and, in parallel,
   **section-05-macos-audio** (Rust-core only) and
   **section-06-crash-safe-recovery** (sidecar boot path).
4. **section-04-qml-screens** (after 03).
5. **section-07-packaging** (after 02+03+05 exist; do the EARLY macOS
   packaging spike near the start of phase 3, not at the end).
6. **section-08-testing-ci** is built incrementally alongside 02–07 and gates
   the ship.

## Section Summaries

### section-01-branch-setup
Salvage prototype-only artifacts (PROTOTYPE.md verdict — already in project
memory), create `feat/qt-migration` from `feat/compose-desktop-rewrite` (so
`ui-compose/` is present as the behavior reference), delete the local-only
`proto/jewel-look-feel`. Consciously reverses the Section-06 "keep the branch"
decision (owner instruction 2026-05-18). Nothing pushed.

### section-02-sidecar-server
Harden the existing `app/src/cli.rs Serve` into `meeting-server`: ephemeral
port 0 + single stdout handshake line (logs to stderr only), bearer-token auth
middleware, strict loopback assertion, `/health`, `/version` with a single
source of truth for `PROTOCOL_VERSION`, orphan reaping (inherited pipe on POSIX
/ Job Object on Windows), reuse the existing singleton flock, graceful
shutdown. Critical path.

### section-03-qt-skeleton
New `qt-app/` (CMake, Qt 6.7+). `QQuickStyle::setStyle("Fusion")` + compiled
`qtquickcontrols2.conf`; no Material/Universal plugins. `SidecarManager`
(QProcess spawn, parse handshake, `/health` gate, terminate→kill, Windows Job
Object), version-range gate (Q9), `ApiClient` over QNetworkAccessManager with
bearer token + `JobPoller`. Top-level `run-qt.sh` dev entrypoint.

### section-04-qml-screens
Port screen behavior/flows from frozen Compose `ui-compose/` (NEVER visual
design). Includes a ViewModel-audit step (map MeetingListVM/RecordingVM/
ProtocolGenerateVM logic → core/API vs Qt client). Markdown protocol rendering
is a first-class deliverable. All four list data-states; full
record→transcribe→protocol round-trip.

### section-05-macos-audio
Replace the macOS `record_system` stub with ScreenCaptureKit audio-only capture
via the `screencapturekit` crate (macOS 13.0 floor). No real screen output
(`minimumFrameInterval` workaround only). Mixed-audio clock-drift spike +
alignment/resampling. TCC re-prompt UX (ad-hoc-signed v1). Parallel to UI work.

### section-06-crash-safe-recovery
On core startup, find orphaned in-progress recordings and reconstruct the WAV
header by parsing the real RIFF chunk list (NOT assuming a 44-byte header — the
hound float WAV has a `fact` chunk) and recompute `data` size from the file;
reconcile/recreate the DB row from the slug. Idempotent.

### section-07-packaging
Two-binary bundle on macOS (deep-signed ad-hoc `.app` + Homebrew Cask
`no_quarantine` + self-hosted-tap fallback; Sept-2026 unsigned-cask sunset
tracked), Linux (AppImage), Windows (windeployqt + installer). Includes the
EARLY end-to-end macOS spike (clean macOS 13 + 15) done near phase-3 start.

### section-08-testing-ci
Sidecar contract tests (handshake/auth/version-gate/reaping/singleton),
recovery-pass tests (idempotent, both orphan kinds), offscreen Qt smoke,
3-OS CI build matrix, protocol-version equality CI check. Gates the ship.
