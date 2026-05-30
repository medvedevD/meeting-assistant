# Qt Migration — Interview Transcript

Date: 2026-05-18. Scope: sequencing / v1 boundaries / resolving the research-surfaced
Q3×Q4 tension. Architecture (the 11 locked decisions) was NOT re-litigated.

---

### Q1 — Q3×Q4 signing tension (research-surfaced)

**Context:** ScreenCaptureKit needs a stable signing identity (ad-hoc signing
changes the identity hash every build/update → macOS re-prompts for the
Screen-Recording TCC grant each time). Homebrew is deprecating unsigned/
un-notarized casks ~Sept 2026.

**Answer: Accept the TCC re-prompt.** macOS v1 ships ad-hoc/self-signed; the
user re-grants Screen-Recording permission after each update. Cheapest path,
consistent with the standing Q3 decision (no paid Apple Developer ID for now).

**Consequences for the plan:**
- The Homebrew "unsigned cask sunset ~Sept 2026" is a **tracked risk with a hard
  date**, not a hypothetical. Buying the Apple Developer ID remains the eventual
  exit and is the documented mitigation; a **self-hosted Homebrew tap** is the
  fallback distribution channel if homebrew-cask eligibility is lost.
- The macOS audio section MUST surface the re-prompt UX explicitly (first-run
  and post-update permission flow, clear in-app guidance) so the known rough
  edge is at least handled gracefully.

### Q2 — First milestone / critical path

**Answer: Sidecar-hardening first.** Research confirmed a loopback
`axum::serve` already exists in `app/src/cli.rs` (`Serve` command). The first
milestone is hardening that into a real sidecar (port 0 + stdout port handshake,
`/health`, `/version` + protocol-version range, bearer token, orphan reaping,
strict-loopback assertion). It de-risks the integration boundary and unblocks
the QML UI work.

### Q3 — v1 scope: explicit fast-follow (NOT v1-blocking)

All four deferred to fast-follow:
- **Crash-friendly recording format (Q5 option c)** — v1 = recovery-pass only.
- **UDS / named-pipe transport** — v1 = loopback TCP only; UDS is plan B only if
  a concrete problem surfaces.
- **Native macOS/Windows QML styles** — v1 = Fusion only.
- **Auto-update (Sparkle/WinSparkle/AppImageUpdate)** — v1 = manual reinstall /
  `brew upgrade`; two-binary auto-updater is fast-follow.

### Q4 — Design spec (IntelliJ reference) timing

**Answer: Separate workstream, later.** This plan covers **engineering only**.
The design-spec doc + owner-driven visual-iteration loop are NOT a section here.
The QML-UI section still enforces Fusion and ports screen *behavior/flows* from
the frozen Compose app, but defers final visual design to the later design-spec
workstream (consistent with the earlier owner statement).

---

## Net effect on section structure

- Design-spec section is **dropped** from this plan (separate workstream).
- v1 sections are scoped to: sidecar hardening (FIRST/critical path) → QML+Fusion
  UI (behavior-ported) → macOS ScreenCaptureKit audio (parallel, with re-prompt
  UX) → crash-safe recovery (recovery-pass only) → packaging ×3 (ad-hoc/self-
  signed macOS + Homebrew-sunset risk tracked).
- Fast-follow items are recorded in the plan as an explicit out-of-v1 list, not
  interleaved into v1 sections.
