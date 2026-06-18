# Alpha release — verify correctness on 3 OSes

## Problem

The app builds and passes the offscreen smoke/contract gate on Linux/macOS/Windows
(`.github/workflows/qt-ci.yml`), and packaging scripts that produce real
installers exist (`packaging/{macos,linux,windows}`). But nothing ties them
together: CI uploads the *deployed tree* `dist/artifact`, not the user-facing
installers, and there is no tagged GitHub Release. We need a first prerelease
(`v0.1.0-alpha.1`) that produces installable builds on all three OSes so the
author can self-verify real runtime behaviour — mic capture, macOS TCC
permissions, Whisper transcription, LLM protocol generation, audio playback —
on actual hardware (the CI gate only proves it boots offscreen).

**Audience: the author only.** This alpha is a personal cross-OS self-test, not
an external tester distribution — which removes onboarding docs, an issue
template, and any shared-key handling from scope, but makes access to real
Windows + Linux machines the gating dependency (the author has macOS locally).

## Scope

### In
- Tag/version scheme for prereleases (`v0.1.0-alpha.1`).
- A tag-triggered release workflow that runs the existing packaging scripts on
  each OS and publishes a GitHub **prerelease** with the three installers.
- **Arch coverage = CI runner defaults**: macOS `arm64`, Windows `x64`, Linux
  `x86_64`. No Intel-mac / universal build in this alpha.
- A personal per-OS smoke-test checklist covering the **full happy path**
  (acceptance bar), plus a short note to self on the Gatekeeper/SmartScreen
  open-past steps for the Windows/Linux machines.
- Diagnostics path: confirm/record where `meeting-server` writes logs so a
  failed run on the Windows/Linux box is debuggable.

### Out
- **External tester distribution** — audience is the author only: no public
  onboarding doc, no issue template, no shared API key. Each run uses the
  author's own Anthropic key entered in Settings.
- **Code signing / notarization** — deferred fast-follow. Alpha ships ad-hoc
  (macOS) and unsigned (Windows/Linux); bypass is documented. (See
  `packaging/macos/HOMEBREW-SUNSET.md`, `packaging/windows/SMARTSCREEN.md`.)
- **Auto-updater** — not in alpha; re-download to update.
- **Homebrew Cask / winget / repo distribution** — GitHub Release only for now.
  A release-attached `install-macos.sh` is allowed as a personal alpha install
  helper because it pins and verifies this exact release DMG before automating
  the documented macOS quarantine bypass.
- **Intel-mac (x86_64) / universal binaries** — arm64 only this round.
- New app features. This is purely a release-engineering task.
- Apple Developer ID enrollment and any CI secrets.

## Pre-flight (catch bugs before the cross-OS cycle)

The existing suite is already strong where automated tests belong (47 test files:
core use-cases, adapters incl. wiremock LLM + whisper + audio + worker, the
app/api IPC contract — handshake, auth, lifecycle, job progress — and the Qt
offscreen smoke driving the real `SidecarManager`/`ApiClient`). So there is **no
test-writing phase** here: more unit/integration tests would cover the wrong
layer relative to what the alpha exists to find (real mic/audio, TCC, packaging,
real Whisper/LLM, live QML). Instead, run these cheap gates *before* tagging, so
an expensive cross-OS cycle isn't burned on a dumb logic bug:

1. **Green CI on the exact commit to be tagged**, plus a local full-workspace run
   `cargo test --manifest-path rust/Cargo.toml` and `ctest --test-dir qt-app/build`.
2. **End-to-end `./run-qt.sh` on macOS** before any packaging — exercises the
   real GUI + sidecar in the dev loop and catches logic/QML regressions without
   the installer round-trip. Cheapest "catch on shore" available.
3. *(Optional, only if assertable without real audio)* a narrow regression test
   for the surface that landed this week and is untested — the in-card audio
   player (play/seek/speed/volume) and audio-source selection. If verifying it
   needs live audio, skip it here and catch it manually in the smoke checklist.

### Bug-handling policy during the alpha

Every bug found during pre-flight **or** the cross-OS self-test must be covered
by a regression test before the next tag — per `RULES.md` "Regression Tests": add
or update a test at the **lowest level that reproduces the failure**, it must
fail on the old behaviour and pass with the fix, and QML/screen-behaviour bugs
get a QML integration test rather than only a C++ unit test. The fix and its
regression test ride together; a fix does not go into the next `-alpha.N` tag
without its test. This turns the manual alpha into durable coverage so the same
break cannot silently regress on beta.

## Deliverables

### Files
- `.github/workflows/release.yml` *(new)* — triggers on `push: tags: ['v*']`
  (and `workflow_dispatch`). Reuses the already-debugged Qt/Rust install steps
  from `qt-ci.yml` (uv-driven aqtinstall in ubuntu:20.04 container for the
  glibc floor; `install-qt-action` on macOS/Windows host runners). Per OS:
  - Linux: `packaging/linux/build-appimage.sh` → `MeetingAssistant-<arch>.AppImage`
  - macOS: `packaging/macos/build-app.sh` → `MeetingAssistant-<ver>.dmg`
  - Windows: install Inno Setup (e.g. `choco install innosetup`), then
    `packaging/windows/build-installer.ps1` → `MeetingAssistant-Setup-<ver>.exe`
  - Final job: `gh release create "$TAG" --prerelease --title … --notes-file …`
    attaching the three installers plus a generated macOS installer
    (`install-macos.sh`) that pins the DMG URL/SHA, verifies the download and
    code seal, installs into `/Applications`, and removes quarantine for the
    personal alpha install path.
- `plans/alpha-release/smoke-checklist.md` *(new, side file)* — personal per-OS
  manual pass for the **full happy path** acceptance bar: launch → sidecar
  handshake → mic permission prompt → record → stop → transcription job
  completes → enter own Anthropic key → generate protocol → audio playback
  (play/seek/speed/volume) → settings persist across restart. Includes the
  one-liner open-past-Gatekeeper/SmartScreen reminder and the log file path, so
  the same doc is the runbook on the Windows/Linux machines. (No public
  TESTING.md / issue template — audience is the author.)

### Investigation (DONE)
- Sidecar logs go to **stderr only** (confirmed; contract test
  `logs_go_to_stderr_only_never_stdout`). The GUI relays them via `qInfo`, but a
  Finder/Explorer/.desktop launch has no terminal so they were discarded.
  **Resolved**: added a GUI file logger (`qt-app/src/Logging.{h,cpp}`, wired in
  `main.cpp` + CMake) teeing all Qt + relayed sidecar output to a per-user log
  with `.prev` rotation; and `with_ansi(stderr.is_terminal())` in
  `meeting-server.rs` so the relayed lines are plain text in the file (colour
  only on a dev tty). Verified at runtime (Ready reached, 0 ANSI escapes) and
  the 15 sidecar-contract tests still pass.
- Packaging scripts run headless: all three discover Qt via `CMAKE_PREFIX_PATH`,
  have no interactive prompts; macOS DMG via `hdiutil`, ad-hoc `codesign -s -`
  (no keychain/secrets). `release.yml` supplies the CI-only bits: Linux
  `APPIMAGE_EXTRACT_AND_RUN=1` (no FUSE in container), Windows `choco install
  innosetup` + `ilammy/msvc-dev-cmd` (the script prefers ninja → needs cl.exe).

### Test plan
- Release workflow is itself the test surface; validate via `workflow_dispatch`
  on a throwaway pre-tag before the real tag, so a broken packaging step doesn't
  burn a version number.
- Acceptance: a `v0.1.0-alpha.1` prerelease exists with exactly three installer
  assets (macOS arm64 `.dmg`, Windows x64 `Setup.exe`, Linux x86_64 `.AppImage`);
  the author installs each on a real machine of its OS (mac locally;
  Windows/Linux via VM or hardware) and completes the **full happy path** in
  `smoke-checklist.md`, including protocol generation with the author's own key.
- No *proactive* new automated tests for the release plumbing — `qt-ci.yml`
  stays the build-correctness gate; this task adds packaging + distribution on
  top. *Reactive* tests are still required: any bug surfaced during pre-flight or
  self-test gets a regression test per the bug-handling policy above.

## Decisions
- **Distribution channel: GitHub Release (prerelease).** Chosen over reusing raw
  CI artifacts (not installers, require GitHub login, 14-day retention) and over
  manual hand-off (doesn't scale). Tag-driven, fits existing `gh`/CI.
- **Audience = author only (self-test).** Acceptance is the author running the
  full happy path on each OS; no external testers this round. Drops onboarding
  doc, issue template, shared-key handling.
- **Acceptance bar = full happy path**, not just install-and-launch: record →
  transcribe → generate protocol → playback → settings persistence must work on
  each OS. Protocol generation uses the author's own Anthropic key.
- **Arch coverage = CI runner defaults** (macOS arm64 / Windows x64 / Linux
  x86_64). Intel-mac and universal builds are explicitly deferred; if the author
  only has an Apple-Silicon Mac this is full coverage, an Intel Mac would not be
  exercised.
- **Ship unsigned/ad-hoc for alpha.** Signing/notarization is the documented
  fast-follow; gating the alpha on a paid Apple Dev ID would block the only goal
  that matters now — observing real behaviour on three OSes.
- **Reuse, don't fork, the CI build steps.** The Qt/Rust install dance in
  `qt-ci.yml` was hard-won (glibc floor, xcb plugins, aqtinstall under focal);
  `release.yml` must copy those steps verbatim rather than re-derive them.

## Risk / dependency
- **Gating dependency #1: real or virtual Windows + Linux machines.** The author
  self-tests the full happy path, so the alpha is not "done" until it has been
  run on actual Windows and Linux (mac is available locally). This must be lined
  up before tagging — it is the critical path to acceptance, not a side risk.
- Sidecar log visibility: if `meeting-server` only logs to stderr it is
  swallowed by the GUI-spawned process and a failure on the remote OS box is
  undebuggable — hence the investigation step above.
