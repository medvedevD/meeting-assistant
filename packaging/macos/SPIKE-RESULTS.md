# EARLY macOS packaging spike — results

Mandated by section-07 as the single most fragile area: *ad-hoc sign + nested
helper deep-sign + macdeployqt + Gatekeeper + ScreenCaptureKit TCC*. Run near
the start of phase 3 so a dead-end here is found before sections 03–05 go deep.

**Run:** 2026-05-19 · host **macOS 26.5, Apple Silicon (arm64)** · Qt 6.11.1
(official open-source LGPLv3 build, `~/Qt/6.11.1/macos`) · harness
`packaging/macos/build-app.sh`.

Done with the **real** GUI + **real** sidecar (sections 02/03 already exist),
which is a strictly stronger test than the stub-GUI/stub-sidecar the section
allowed.

## Result: PASS (chain de-risked) — two clean-OS blockers triaged below

| Step | Outcome |
|---|---|
| `cargo build meeting-server` + `cmake` build of `MeetingAssistant.app` | ✅ |
| `cmake --install` places helper **inside** `Contents/MacOS` + licenses in `Contents/Resources/licenses` | ✅ |
| `macdeployqt` bundles the Qt runtime (46 frameworks + plugins; helper untouched, as expected) | ✅ |
| Ad-hoc **inside-out** deep-sign (frameworks → dylibs/plugins → helper → main → bundle) | ✅ |
| `codesign --verify --deep --strict --verbose=2` → *valid on disk, satisfies its Designated Requirement* | ✅ |
| `spctl --assess` → **rejected** (ad-hoc, un-notarized) | ✅ *expected* — Homebrew Cask `no_quarantine` is the launch path |
| `Info.plist`: `NSScreenCaptureUsageDescription` + `NSMicrophoneUsageDescription` + `LSMinimumSystemVersion 13.0` present and covered by the seal | ✅ |
| Bundled **signed** `meeting-server` runs from inside the `.app`, emits the one-line handshake (`port/token/protocol/build`) | ✅ |
| Signed `.app` launched via `open`: GUI (pid P) spawned sibling `meeting-server --parent-pid P --parent-pipe-fd 5` from `Contents/MacOS` | ✅ |
| On quit: sidecar reaped, **no orphan** (parent-death linkage works in the bundle layout) | ✅ |

The decisive risks the early spike exists to catch — *does ad-hoc deep-signing
a **two-binary** Qt bundle even produce a valid seal? does `macdeployqt` + a
nested helper load past dyld's signature check? does the signed bundle launch
and locate+spawn its sibling helper?* — are all **PASS** on macOS 26.5.

## Triaged blockers (not verifiable on this host — recorded per acceptance)

1. **Clean macOS 13 AND macOS 15 machines — unavailable.** Host is macOS 26.5;
   no 13/15 box or VM here. *Triage:* the chain-level dead-ends are cleared on
   26.5; residual 13/15 risk is OS-version-specific UI/policy, not a packaging
   redesign: (a) Gatekeeper dialog wording differs but `no_quarantine` removes
   the quarantine bit so launch path is identical; (b) Sequoia (15) adds a
   30-day Screen-Recording re-confirmation + persistent orange indicator —
   already known/accepted (section-05 TCC-reprompt UX). **Exit:** verify on
   macOS-13/15 GitHub Actions runners or a physical/VM pass in section-08 CI
   before tagging v1. **Not a blocker for proceeding with 03–05.**

2. **Interactive Screen-Recording TCC prompt — needs a real recording in an
   interactive WindowServer session** (this shell is headless: the GUI loads
   the bundled `cocoa` plugin and spawns the sidecar, but driving a recording
   needs a desktop session). *Triage:* section-05 already verified **live on
   macOS 26.5** that the SCK path raises and handles the grant; section-07's
   only contribution to that path is the `Info.plist` key, now verified
   **present and inside the signature** — exactly what prevents the
   "process killed with no prompt" failure mode. No new risk.

## Hard lessons folded back into the scripts

- `codesign --entitlements` uses AMFI's restricted plist parser: **XML comments
  are rejected** (`AMFIUnserializeXML: syntax error`), and `--` / non-ASCII in
  a comment is doubly fatal. `entitlements.plist` is kept **comment-free**; the
  rationale lives in `codesign-deep.sh` and the README.
- Signing must be **strictly inside-out**; `codesign --deep` is avoided per
  Apple guidance (it does not reliably seal nested helpers). The explicit
  frameworks → dylibs → helper → main → bundle order is what makes
  `--verify --deep --strict` pass for a two-binary bundle.
- `macdeployqt` confirmed to **not** copy the helper — the CMake `install()`
  rule is mandatory and is what the spike validated.
