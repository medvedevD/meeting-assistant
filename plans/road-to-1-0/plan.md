# Road to 1.0: trust and polish

## Problem

The app is at `v0.1.0-alpha.16` and works end to end, but a new user still meets alpha
friction: Gatekeeper and SmartScreen warnings, a Screen Recording permission that breaks
on every macOS update, no way to learn that a new version exists, a recording meter that
moves when nothing is captured, and a repository with no contributor entry points. These
block open-source adoption more than any missing feature does.

## Scope

**In**

- Recording confidence: real level meter, device change during a recording.
- Trusted install: Homebrew as the first-class macOS path, a decision on Windows
  signing, an in-app notice when a new release is out.
- First run: every onboarding step is actionable and shows whether it is done.
- Visual polish: dark theme, one consistency pass over every screen, new screenshots.
- Contributor readiness: `CONTRIBUTING.md`, issue and PR templates, `SECURITY.md`,
  code of conduct, a guide for contributing minutes templates.
- The 1.0 release itself: stop publishing as prerelease, exit criteria below.

**Out**

- New product features: search, speaker labels, chat with meetings, integrations.
- Anything paid or hosted.
- Developer ID signing and notarization on macOS (see Decisions). The steps stay
  documented under "Signing and distribution" in `packaging/README.md`.
- A self-installing updater (Sparkle, WinSparkle, AppImageUpdate). 1.0 ships a notice
  with a link; Homebrew users update through `brew`.
- Native macOS and Windows QML styles; Fusion stays the base.
- Renaming internal `Meety*` identifiers.

## Deliverables

Phases are ordered. Each one is promoted to its own `plans/<slug>/` when it starts;
this file stays as the umbrella and is removed by the commit that ships 1.0.

### Phase 1. Recording confidence

Two existing backlog items, promoted as they are.

| Slug | Change |
|---|---|
| `recording-level-meter` | Expose the level of the active capture session from the sidecar; bind `NewRecordingScreen.qml` to it in place of the `Math.sin` animation. Reuse `MeetyLevelMeter`. |
| `audio-device-hot-change` | Detect a lost stream in the capture adapters, keep the audio already recorded, fall back to the OS default or stop safely, report it on the recording status line. |

### Phase 2. Trusted install and updates

- **macOS stays ad-hoc signed, Homebrew first.** README and `docs/INSTALL*.md` lead
  with the `brew` command and present the DMG with *Open Anyway* as the second path.
  The Screen Recording grant still breaks on every update, so the recovery has to be
  smooth: after an update, detect the stale grant before the user starts a recording
  and offer **Reset permission** right there, instead of failing the recording first.
  Check on a real update whether the keychain prompts for saved API keys can be cut
  to one.
- **Windows signing.** Pick a route (see Decisions), wire it into
  `packaging/windows/build-installer.ps1` and `release.yml`, update
  `packaging/windows/SMARTSCREEN.md`.
- **Update notice.** New sidecar route that reads the latest GitHub release and
  compares it with the build version; a banner in `AppShell.qml` with a link to the
  release; a switch in Settings to turn the check off; a line in the README privacy
  section, because this is the app's first network request the user did not start.

### Phase 3. First run

Promoted to [`first-run-onboarding`](../first-run-onboarding/plan.md), which also
covers what later launches check and show.

### Phase 4. Visual polish

- **Dark theme.** `qt-app/qml/theme/Theme.qml` has the `dark` flag reserved. Add the
  dark palette, follow the OS setting, add an override in Settings, check every screen
  in `ThemeGallery.qml` and in the app.
- **Consistency pass.** Go screen by screen for spacing, type sizes, empty, loading and
  error states, keyboard focus order, and window resize down to the compact sidebar.
- **Split the two largest QML files** while touching them: `NewRecordingScreen.qml`
  (921 lines) and `settings/WhisperPanel.qml` (about 1,350 lines).
- New `docs/screenshots/` and demo GIF in both themes.

### Phase 5. Contributor readiness

Can run in parallel with phases 3 and 4.

- `CONTRIBUTING.md`: build, test, branch flow (feature → `develop` → `main`), the
  regression-test rule, how plans and backlog work.
- `.github/ISSUE_TEMPLATE/` (bug with log locations, feature idea), a PR template.
- `SECURITY.md`, `CODE_OF_CONDUCT.md`.
- `prompts/README.md`: how to add or translate a minutes template. This is the
  cheapest first contribution.
- "Add an LLM provider" section in `docs/ARCHITECTURE.md`, pointing at the adapter
  interface and one existing provider as the example.
- Turn the remaining backlog files into labelled GitHub issues (`good first issue`,
  `help wanted`).

### Phase 6. Release 1.0

- `release.yml`: publish stable tags as a full release, keep `--prerelease` for
  `-alpha`/`-beta` tags.
- README: remove the Alpha note. `docs/INSTALL*.md`: keep the macOS walkthrough;
  remove the Windows one only if Windows gets signed.
- One or two `-beta` tags through `packaging/SMOKE-CHECKLIST.md` on all three OSes.

**Exit criteria for 1.0**

1. A fresh macOS install through Homebrew opens with no Gatekeeper step.
2. After a macOS update, the app itself leads the user back to working system-audio
   recording, with no Terminal and no failed recording.
3. A user with no prior setup reaches their first minutes following only the app.
4. The recording screen shows silence as silence.
5. Unplugging the microphone mid-recording loses no recorded audio.
6. A running app tells the user when a newer release exists.

### Stretch, not gating 1.0

`whisper-gpu-release-build`: a Vulkan build for Windows and Linux. It adds a packaging
lane and a new class of driver bugs, so it lands in 1.0 only if phases 1 to 5 are done.

### Test plan

No test-writing phase ahead of the release. Each phase adds tests for its own changes
at the lowest level that shows the behaviour, per the regression rule in `RULES.md`.
Signing, notarization, permissions and real devices are verified by the manual smoke
cycle, with green CI on the tagged commit as the pre-flight gate.

## Decisions

Decided:

1. **No Apple Developer Program for 1.0** (owner, 2026-11-14). macOS ships ad-hoc
   signed and Homebrew-first. Cost accepted: DMG users meet Gatekeeper, and every
   update resets the Screen Recording grant. Revisit after 1.0, when there is an
   audience to justify the fee.

Open, owner's call:

2. **Windows signing route.** Candidates to price and check eligibility for: SignPath
   Foundation's free programme for open-source projects, Azure Trusted Signing, a
   classic OV certificate. Not researched yet. Shipping 1.0 unsigned on Windows with
   the one-click SmartScreen step is an acceptable fallback.
3. **Dark theme in 1.0 or after.** It is the largest item in phase 4. Recommendation:
   in, since users of a desktop tool expect it and screenshots in both themes help the
   README.
4. **Update check default.** Recommendation: on by default, disclosed during onboarding
   and in the privacy section, one switch to turn it off.
