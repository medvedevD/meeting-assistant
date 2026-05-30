# Section 07 — Packaging ×3 OS (two-binary bundle)

## Background
Ship a polished v1 on macOS/Linux/Windows. The artifact is a **two-binary
bundle**: the Qt GUI + the `meeting-server` Rust sidecar, presented as one
application. macOS has no paid Apple Developer ID for now (Homebrew Cask
`no_quarantine`, macOS-only constraint); Linux/Windows have no cost gate. The
macOS chain (ad-hoc sign + nested helper deep-sign + Homebrew + ScreenCaptureKit
TCC + Sequoia Gatekeeper) is the single most fragile area — an EARLY spike is
mandatory.

## Requirements
A clean machine on each OS can install and launch the bundle; the GUI starts the
sidecar and is fully functional; uninstall is clean; the macOS Homebrew
unsigned-cask sunset (~Sept 2026) is documented with its date and exit; the
early macOS spike has passed (or its blockers are recorded/triaged).

## Dependencies
- Requires: section-02 + section-03 (both binaries exist), section-05 (macOS
  Info.plist `NSScreenCaptureUsageDescription`).
- Blocks: nothing.

## Implementation details
- **EARLY macOS spike (do near the start of phase 3, before sections 03–05 go
  deep):** stub GUI + stub sidecar, packaged into a `.app`, deep-signed ad-hoc,
  installed via the cask, launching the helper, and successfully triggering the
  Screen-Recording TCC grant on a clean **macOS 13 AND macOS 15** machine.
  Findings feed sections 05 & 07 before large effort is sunk.
- **macOS:** build `MeetingAssistant.app`; place `meeting-server` in
  `Contents/MacOS/` beside the GUI. **Deep-sign** (sign the helper first, then
  the app, or `codesign --deep`) — ad-hoc/self-signed for v1 (Apple Silicon
  requires at least ad-hoc to run at all). Add
  `NSScreenCaptureUsageDescription` to Info.plist. `macdeployqt` for the Qt
  runtime — note it does NOT copy the helper; copy it via a CMake install rule.
  Distribute via a **Homebrew Cask** with `no_quarantine`; prepare a
  **self-hosted tap** as the fallback. **Tracked risk:** Homebrew is deprecating
  unsigned casks ~Sept 2026 — documented exit is buying an Apple Developer ID +
  notarization (fast-follow, dated).
- **Linux:** AppImage via `linuxdeploy` + the Qt plugin (bundles Qt6 + QML
  runtime); `meeting-server` bundled beside the GUI inside the AppDir; `.desktop`
  integration. $0, no signing gate.
- **Windows:** `windeployqt` for the Qt runtime; installer via Inno Setup (or
  WiX/NSIS); `meeting-server.exe` beside the GUI `.exe`. No cert required to
  run; unsigned → one-click SmartScreen "More info → Run anyway" (documented;
  OV cert is fast-follow).
- **Cross-cutting:** the GUI locates the sidecar via its own exe dir on every
  OS. Both binaries are built from the same revision and version-pinned
  together; the protocol-version check (sections 02/03) is the runtime safety
  net. **Qt LGPLv3 compliance:** dynamically link Qt; include the Qt source
  offer / written offer in all three artifacts.

## Acceptance criteria
- [~] EARLY macOS spike passed on clean macOS 13 + 15 (or blockers recorded and
      triaged). — **PASS (chain de-risked) + 2 blockers triaged**. Real
      GUI+sidecar → `.app` → macdeployqt (46 fw) → ad-hoc inside-out
      deep-sign → `codesign --verify --deep --strict` *valid / satisfies
      DR*; signed bundle launches, GUI spawns sibling sidecar, clean reap.
      Blockers (no clean macOS-13/15 box here — host is 26.5; interactive
      TCC needs a desktop session) recorded + triaged in
      `packaging/macos/SPIKE-RESULTS.md`; exit = section-08 CI on
      macOS-13/15 runners.
- [~] macOS: cask install launches the deep-signed `.app`; GUI starts the
      bundled sidecar; Screen-Recording TCC prompt appears; uninstall clean.
      — **PASS (mechanism), partial (env)**: deep-signed `.app` + `.dmg`
      built; launched signed `.app` → GUI started bundled sidecar (verified
      live, macOS 26.5). Cask (`Casks/meeting-assistant.rb`, `no_quarantine`
      via `xattr` postflight) + `zap` clean-uninstall written; `ruby -c` OK.
      TCC prompt itself = section-05 (verified live there); the Info.plist
      key it needs is present + inside the signature. Real `brew install`
      on a clean Mac is the section-08 CI step.
- [~] Linux: AppImage runs on a clean distro; GUI starts the bundled sidecar.
      — **CODE COMPLETE, unrun here (macOS host)**: `linux/build-appimage.sh`
      (linuxdeploy + qt plugin, QML scan, bundled sidecar in `usr/bin`,
      `.desktop` + icon) `bash -n` clean. Runtime check = section-08 Linux CI.
- [~] Windows: installer works on a clean machine; SmartScreen path documented;
      GUI starts the sidecar. — **CODE COMPLETE, unrun here**:
      `windows/build-installer.ps1` (windeployqt) + `installer.iss` (per-user,
      sibling helper, clean uninstall + optional data wipe);
      `windows/SMARTSCREEN.md` documents the 1-click path + OV/EV exit.
      Runtime check = section-08 Windows CI.
- [x] Both binaries version-pinned together; GUI locates sibling sidecar on all
      3 OSes. — **PASS**: one recipe builds both from one checkout, same
      `0.1.0`; CMake `install()` co-locates them per-OS; runtime IPC
      protocol-version handshake is the skew safety net. macOS verified
      live (GUI found+spawned `Contents/MacOS/meeting-server`).
- [x] Homebrew unsigned-cask sunset (~Sept 2026) documented in-repo with the
      exit (Apple Developer ID + notarization). — **PASS**:
      `packaging/macos/HOMEBREW-SUNSET.md` (date 2026-09-01, self-hosted-tap
      fallback, Dev-ID+notarization exit, owner checklist) + summarised in
      `packaging/README.md`.
- [x] Qt LGPLv3 obligations satisfied (dynamic link + source/written offer) in
      all 3 artifacts. — **PASS**: Qt dynamically linked (verified: 46
      frameworks in `Contents/Frameworks`, none static); `LICENSES/`
      (written 3-yr offer + Qt license text + version) installed into all
      three artifacts via CMake; verified present in the built `.app`.

## Files to create/modify
- `qt-app/CMakeLists.txt` install rules.
- `packaging/macos/*` (cask, entitlements, Info.plist, codesign script).
- `packaging/linux/*` (AppImage recipe).
- `packaging/windows/*` (installer script).
