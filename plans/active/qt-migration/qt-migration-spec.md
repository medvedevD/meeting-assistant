# Qt Migration — Spec (gepetto input)

> Planning directory: `.claude/plans/qt-migration/` (per repo CLAUDE.md — one folder
> per task; this OVERRIDES gepetto's default location). All gepetto-generated
> artifacts (`claude-research.md`, `claude-interview.md`, `claude-spec.md`,
> `claude-plan.md`, `reviews/`, `sections/`, etc.) must be written here.

## Goal

Migrate the Meeting Assistant desktop UI from Kotlin/Compose to **Qt Quick / QML**,
keeping the existing Rust core intact. Produce a sectionized implementation plan
for a v1 that ships, polished, on **macOS / Linux / Windows**.

Background: the Jewel look-and-feel prototype (`proto/jewel-look-feel`) closed the
Qt-vs-Compose question — verdict **migrate to Qt** (both ported screens still read
as mobile / amateurish even after desktop-ing attempts). The Rust + Kotlin stack's
only remaining unsolved problem was UI look & feel, which is a UI-layer problem;
the Rust core is sound and stays.

## Locked decisions (constraints — NOT open for re-litigation)

These were resolved in a grill session on 2026-05-18 and recorded in project
memory (`project_goal_shipping.md`, sections marked "DECIDED 2026-05-18"). The
plan must treat them as fixed inputs.

1. **Integration boundary — sidecar HTTP.** Rust core stays whole; C++ rewrite of
   the core is rejected. The Qt UI talks to the Rust core over **loopback
   `127.0.0.1`** (strict bind, port 0 = OS-chosen, bearer token) against the
   existing Axum `meeting-api`. Unix-domain-socket / named-pipe transport is the
   in-pocket plan B (costs a ~50-line `QLocalSocket` HTTP shim since
   `QNetworkAccessManager` has no UDS). NOT cxx-qt / C-ABI.
2. **Crash isolation.** A panic/segfault in the core (or whisper.cpp under it)
   must NOT crash the GUI process holding a live recording.
3. **macOS signing — provisional, macOS-only.** Start with Homebrew Cask
   (`no_quarantine`), NO paid Apple Developer ID for now (revisit later). Linux
   ($0, no gate) and Windows ($0, one-click SmartScreen only) are unaffected.
4. **macOS system-audio in v1.** Implement macOS system/loopback capture via
   **ScreenCaptureKit / Core Audio taps** (macOS 13+) inside the Rust
   `AudioCapture` adapter. v1 must be feature-equal on all 3 OSes. QtMultimedia
   is irrelevant here.
5. **Crash-safe recording.** v1: on core startup, scan for orphaned in-progress
   recordings and reconstruct the WAV header from on-disk data length. Clean
   end-state target (post-v1): a crash-friendly recording format (streaming, no
   trailing finalize).
6. **UI = Qt Quick / QML** (owner's call, learning-motivated, made against the
   Widgets recommendation — recorded as a deliberate trade-off).
7. **Style foundation = Fusion.** Qt Quick Controls `Fusion` style as the v1
   baseline. Default Basic / Material / Universal styles are FORBIDDEN to ship.
   Native macOS/Windows QML styles are an optional later enhancement, not the v1
   foundation. No bespoke control restyling.
8. **Sidecar robustness — mandatory v1:** startup handshake (core prints chosen
   port; GUI waits on `/health` with bounded retry), orphan reaping (core
   self-exits if parent GUI PID dies), version check. Single-instance already
   solved via the existing `SINGLETON_LOCK_FILE` pattern.
9. **Version contract = protocol version, not build-version equality.**
   `/version` exposes build version (informational) + protocol version (compat
   key) + min-supported protocol version. GUI proceeds iff its protocol version
   is within `[core.min_protocol … core.protocol]` even when build versions
   differ. Hard-fail ONLY on a breaking protocol mismatch, with an explicit
   user-facing message. Protocol version bumps only on breaking IPC changes;
   additive changes do not bump it.
10. **Migration strategy = hard cutover.** Compose `ui-compose` is FROZEN (not
    deleted), zero further investment. It is reference for BEHAVIOR / FLOWS /
    domain wiring ONLY — never for visual design.
11. **Visual north-star = Fusion controls + IntelliJ/JetBrains layout** (density,
    IA, navigation, spacing). Design is a separate workstream: a design-spec doc
    under this planning dir + an owner-driven visual-iteration loop (Claude Code
    cannot see a running GUI; owner renders visual verdicts).

## Codebase facts (verified)

- NO runnable server binary exists yet: `rust/crates/api` (`meeting-api`) is
  library-only — `create_router` / `AppState`, no `axum::serve` / `bind`.
- Full adapter graph + background `Worker` (graceful shutdown) +
  `SINGLETON_LOCK_FILE` (single-instance) already exist in
  `rust/crates/ffi/src/app_core.rs`.
- `rust/crates/app/src/cli.rs:199` already constructs `meeting_api::AppState`.
- The Axum router exposes 7 routes covering the full domain flow: recordings
  start/stop, jobs submit/status, transcribe, protocols generate, meetings list.
- Audio: `rust/crates/adapters/src/audio/cpal_capture.rs` records on a dedicated
  OS thread, streams samples to disk incrementally via `hound::WavWriter` +
  `BufWriter`, finalizes the WAV header only on clean `stop`.
- The `AudioCapture` port: `start_session(session_id, output_path, source,
  echo_cancel)` / `stop_session` / `is_active`.

## Required section coverage

The sectionized plan must cover at least:

1. **Sidecar server binary** — lift the AppCore adapter wiring out of
   `ffi/app_core.rs` into a `tokio::main` binary that calls
   `meeting_api::create_router` + `axum::serve` on a loopback listener; reuse the
   Worker + `SINGLETON_LOCK_FILE` patterns.
2. **Sidecar lifecycle & protocol contract** — startup handshake, orphan
   reaping, `/health` + `/version`, protocol-version range check, bearer token,
   strict loopback bind; UDS/named-pipe as documented plan B.
3. **QML + Fusion UI** — Qt Quick app, Fusion style enforced, screen/flow
   inventory ported from the frozen Compose app (behavior only), HTTP client to
   the sidecar.
4. **macOS system-audio (ScreenCaptureKit)** — new Rust `AudioCapture` adapter
   path for macOS 13+ system/loopback capture; Rust-core task, parallel to UI.
5. **Crash-safe recording recovery** — startup scan + WAV-header reconstruction
   from on-disk data length (v1); note the crash-friendly-format target.
6. **Packaging ×3 OS** — macOS Homebrew Cask (`no_quarantine`, ad-hoc sign for
   Apple Silicon), Linux AppImage/.deb, Windows installer + SmartScreen reality;
   two-binary bundle (GUI + core sidecar) on each.
7. **Design spec & owner-driven visual iteration** — IntelliJ/JetBrains layout
   reference doc, Fusion-component mapping, the visual-verdict loop.

## Notes for gepetto

- Treat the 11 locked decisions as non-negotiable inputs; the interview should
  focus on sequencing, critical path, v1-vs-fast-follow scoping, and unknowns
  inside each section — NOT on re-deciding the architecture.
- Cross-reference project memory `project_goal_shipping.md` for rationale.
