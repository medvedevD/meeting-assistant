# Implementation Prompts — Qt Migration (manual, one section at a time)

Paste one block per section into a fresh Claude Code session, in order. Each is
self-contained. Common rules baked into every prompt: work on
`feat/qt-migration`; never push unless explicitly told; `ui-compose/` is frozen
(behavior reference only, never edit/never copy its design); the 11 architecture
decisions in `claude-spec.md` are fixed (do not re-litigate); stop at the
section boundary and report acceptance status (do not auto-continue).

Order: 01 → 02 → (03, 05, 06 in parallel) → 04 → 07 (+ early macOS spike at
phase-3 start) → 08 grows alongside. Section 02 is the critical path.

---

## Prompt — Section 01 (branch setup)

```
Implement Section 01 of the Qt migration.

Read first:
- .claude/plans/qt-migration/sections/section-01-branch-setup.md
- .claude/plans/qt-migration/sections/index.md (dependency graph)

This is the FIRST action, before any code. It deletes the local-only throwaway
branch proto/jewel-look-feel and creates feat/qt-migration off the
production-Compose base (so ui-compose/ is present as the behavior reference).
This consciously reverses the Section-06 "keep the prototype branch" decision —
that reversal is intended and recorded in project memory (owner instruction
2026-05-18).

Guardrails: branch deletion is destructive — confirm the base branch with me
before deleting anything; salvage ui-compose/PROTOTYPE.md per the section; do
NOT push anything.

When done: report each acceptance checkbox in the section as pass/fail. Stop;
do not start Section 02.
```

---

## Prompt — Section 02 (sidecar server — CRITICAL PATH)

```
Implement Section 02 of the Qt migration.

Preconditions: Section 01 done (on branch feat/qt-migration).

Read first:
- .claude/plans/qt-migration/sections/section-02-sidecar-server.md
- .claude/plans/qt-migration/claude-spec.md (fixed architecture)
- The verified baseline it builds on: rust/crates/app/src/cli.rs (the existing
  `Serve` command) and rust/crates/api/ (router + 7 routes, no auth yet).

This hardens the existing loopback `axum::serve` into a real `meeting-server`
sidecar (stdout handshake, stderr-only logs, bearer auth, /health, /version
with a single-source PROTOCOL_VERSION, orphan reaping, singleton, graceful
shutdown). It is the critical path — everything else depends on it.

Guardrails: stdout = handshake line ONLY; do not re-litigate the boundary
(sidecar HTTP, not cxx-qt); no push.

When done: report each acceptance checkbox pass/fail with how you verified
(esp. 401-without-token, loopback-only, parent-death exit, singleton). Stop at
the section boundary.
```

---

## Prompt — Section 03 (Qt/QML skeleton)

```
Implement Section 03 of the Qt migration.

Preconditions: Section 02 done (meeting-server handshake/contract works).

Read first:
- .claude/plans/qt-migration/sections/section-03-qt-skeleton.md
- .claude/plans/qt-migration/claude-spec.md
- .claude/plans/qt-migration/claude-research.md (Qt section: Qt 6.7+,
  QNetworkAccessManager, QProcess, Fusion enforcement, NO cxx-qt)

Build the new top-level qt-app/ (CMake, Qt 6.7+), enforce Fusion, implement
SidecarManager (spawn/handshake/health/kill + Windows Job Object), the Q9
version gate with kClientProtocol GENERATED from Rust PROTOCOL_VERSION, the
ApiClient + JobPoller, and the top-level run-qt.sh dev entrypoint.

Guardrails: plain C++ Qt + separate Rust binary — NO cxx-qt/qt-build-utils;
Fusion only, no Material/Universal plugins; no push.

When done: report each acceptance checkbox pass/fail (esp. orphan-free quit on
all OSes you can test, version-mismatch dialog, run-qt.sh). Stop at the
boundary.
```

---

## Prompt — Section 04 (QML screens)

```
Implement Section 04 of the Qt migration.

Preconditions: Section 03 done (shell + ApiClient + JobPoller).

Read first:
- .claude/plans/qt-migration/sections/section-04-qml-screens.md
- ui-compose/ as the BEHAVIOR/FLOW reference ONLY (never copy its visual
  design; never edit it)

Do the ViewModel audit FIRST (map MeetingListVM/RecordingVM/ProtocolGenerateVM
logic → core/API vs Qt client) and produce the mapping table before writing
screens. Then implement every screen in QML/Fusion via the sidecar API, with
the markdown protocol view as a first-class deliverable.

Guardrails: behavior parity, not design copy; Fusion only; no push.

When done: report the VM mapping table + each acceptance checkbox pass/fail
(esp. full record→transcribe→protocol round-trip, 4 list states, markdown
render). Stop at the boundary.
```

---

## Prompt — Section 05 (macOS ScreenCaptureKit audio — parallel)

```
Implement Section 05 of the Qt migration.

Preconditions: Section 01 done. Rust-core only — independent of 02/03/04 (can
be done in parallel; needs a macOS 13+ machine to verify).

Read first:
- .claude/plans/qt-migration/sections/section-05-macos-audio.md
- .claude/plans/qt-migration/claude-research.md (macOS-audio section:
  screencapturekit crate, footguns, TCC)
- rust/crates/adapters/src/audio/cpal_capture.rs (the macOS record_system stub)

Replace the macOS system-audio stub with ScreenCaptureKit audio-only capture
via the screencapturekit crate. Do the 60-min mixed-audio clock-drift spike
BEFORE assuming the Linux mix approach transfers. NO real screen output. Handle
the TCC re-prompt UX (v1 is ad-hoc-signed).

Guardrails: never request/retain screen frames; macOS 13.0 floor; no push.

When done: report each acceptance checkbox pass/fail + the drift-spike numbers.
Stop at the boundary.
```

---

## Prompt — Section 06 (crash-safe recovery — parallel)

```
Implement Section 06 of the Qt migration.

Preconditions: Section 02 done (recovery runs in the meeting-server boot path).
Can be done in parallel with 03/04.

Read first:
- .claude/plans/qt-migration/sections/section-06-crash-safe-recovery.md
- rust/crates/adapters/src/audio/cpal_capture.rs (hound WAV write path)

Implement the startup recovery pass. CRITICAL: do NOT assume a 44-byte WAV
header — hound float WAV has fmt + fact chunks. Parse the real RIFF chunk list
to find the data chunk offset; verify the parser against a real
codebase-produced hound f32 file with a unit test BEFORE wiring it into boot.

Guardrails: must be idempotent; recovery-pass only (crash-friendly format is
explicitly fast-follow, out of scope); no push.

When done: report each acceptance checkbox pass/fail (esp. the
record→truncate→reconstruct→assert unit test, both orphan kinds,
double-run no-op). Stop at the boundary.
```

---

## Prompt — Section 07 (packaging ×3 OS) + EARLY macOS spike

```
Implement Section 07 of the Qt migration.

IMPORTANT: do the EARLY macOS packaging spike part of this section NOW (near
phase-3 start), even before 03–05 are deep — it is the most likely dead-end.
The full packaging work needs Section 02+03 (both binaries) and Section 05
(Info.plist key).

Read first:
- .claude/plans/qt-migration/sections/section-07-packaging.md
- .claude/plans/qt-migration/claude-research.md (packaging section; Homebrew
  unsigned-cask ~Sept-2026 sunset; ad-hoc deep-sign)

Two-binary bundle on macOS (Homebrew Cask no_quarantine + self-hosted tap
fallback, ad-hoc deep-sign), Linux (AppImage), Windows (windeployqt +
installer). Document the Homebrew sunset risk in-repo with its date and exit.

Guardrails: no paid Apple Dev ID in v1 (accept the TCC re-prompt — already
decided); Qt LGPLv3 = dynamic link + source/written offer; no push.

When done: report the early-spike result first (pass / blockers triaged), then
each acceptance checkbox pass/fail. Stop at the boundary.
```

---

## Prompt — Section 08 (testing & CI — grows alongside)

```
Implement Section 08 of the Qt migration.

Preconditions: targets the output of 02/03/05/06 — implement the relevant
tests as those sections land; finalize the CI matrix once all exist.

Read first:
- .claude/plans/qt-migration/sections/section-08-testing-ci.md

Sidecar contract tests, recovery-pass tests (incl. the real-WAV parser test),
offscreen Qt smoke, 3-OS CI matrix, and the CI check enforcing Rust
PROTOCOL_VERSION == C++ kClientProtocol.

Guardrails: this gates the ship — do not mark the migration done until CI is
green on all 3 OSes; no push unless explicitly told.

When done: report each acceptance checkbox pass/fail and the CI status. This is
the final section — confirm all 8 sections' acceptance criteria are met.
```
