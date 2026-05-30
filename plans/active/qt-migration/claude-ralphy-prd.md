# PRD — Qt Migration (Meeting Assistant)

## How to use

```
ralphy --prd .claude/plans/qt-migration/claude-ralphy-prd.md
# or: cp .claude/plans/qt-migration/claude-ralphy-prd.md ./PRD.md && ralphy
```

## Context

Replace the Kotlin/Compose desktop UI with **Qt Quick/QML (Fusion)**, keeping
the **Rust core whole**. The Qt GUI runs the Rust core as a child **sidecar**
process over **loopback HTTP** (no FFI/cxx-qt). v1 ships polished on
macOS/Linux/Windows with feature parity. All architecture is fixed — see
`.claude/plans/qt-migration/claude-spec.md`. Each task is a self-contained
section file under `.claude/plans/qt-migration/sections/`; implement them in
dependency order, verifying each section's acceptance criteria before moving on.

**Critical path:** Section 01 → 02, then 03/05/06 in parallel, then 04;
07 needs 02+03+05 (do the early macOS spike near phase-3 start); 08 grows
alongside and gates ship. `ui-compose/` stays frozen as behavior reference only.

## Tasks

- [ ] Section 01: branch-setup — delete `proto/jewel-look-feel`, create
      `feat/qt-migration` off the Compose base
      (`sections/section-01-branch-setup.md`)
- [ ] Section 02: sidecar-server — harden `app Serve` into `meeting-server`
      (handshake/auth/health/version/reaping/singleton)
      (`sections/section-02-sidecar-server.md`)
- [ ] Section 03: qt-skeleton — Qt6.7 app, Fusion, SidecarManager, ApiClient,
      version gate, `run-qt.sh` (`sections/section-03-qt-skeleton.md`)
- [ ] Section 04: qml-screens — behavior-port screens from frozen Compose +
      ViewModel audit + markdown view (`sections/section-04-qml-screens.md`)
- [ ] Section 05: macos-audio — ScreenCaptureKit system/mixed capture in the
      Rust core (`sections/section-05-macos-audio.md`)
- [ ] Section 06: crash-safe-recovery — startup WAV-header reconstruction +
      DB reconciliation (`sections/section-06-crash-safe-recovery.md`)
- [ ] Section 07: packaging — two-binary bundle ×3 OS + early macOS spike
      (`sections/section-07-packaging.md`)
- [ ] Section 08: testing-ci — contract/recovery/Qt-smoke tests + 3-OS CI
      (`sections/section-08-testing-ci.md`)
