# Windows SmartScreen — expected behaviour & documented exit

**Status:** accepted for v1 · no code-signing certificate (fast-follow).

## What the user sees on a clean machine

The v1 installer (`MeetingAssistant-Setup-<ver>.exe`) is **unsigned**. On first
download/run, Microsoft Defender SmartScreen shows:

> *"Windows protected your PC — Microsoft Defender SmartScreen prevented an
> unrecognized app from starting."*

This is **one click to bypass**, by design and documented:

1. Click **More info**.
2. Click **Run anyway**.

The app then installs and runs normally (per-user install, no admin/UAC — see
`installer.iss` `PrivilegesRequired=lowest`). No certificate is required to
*run* an unsigned app; SmartScreen only adds this interstitial.

## Why this is acceptable for v1

- It is a single extra click, not a hard block.
- SmartScreen is **reputation-based**: the warning fades as download/run count
  accumulates for a stable publisher + URL. Keeping the same GitHub release URL
  pattern across versions builds that reputation.
- No $0-gate workaround degrades the product (unlike macOS, Windows has no
  per-dev cost to *run* unsigned software).

## Documented exit (removes the prompt)

Buy an **OV** (or **EV**) code-signing certificate and sign both
`meeting-assistant-qt.exe` / `meeting-server.exe` and the installer with
`signtool`:

- **EV cert:** SmartScreen reputation is granted immediately (no warning from
  first download). Highest cost, requires hardware token.
- **OV cert:** removes the "unrecognized publisher" wording; SmartScreen
  reputation still accrues over downloads but from a named publisher.

Fast-follow steps when the cert is acquired (a script change, not a redesign):

1. In `build-installer.ps1`, after `cmake --install` and after Inno Setup, add
   `signtool sign /tr <timestamp-url> /td sha256 /fd sha256 /a <files>` for the
   two `.exe`s (pre-windeployqt is fine) and the final installer `.exe`.
2. Set `installer.iss` `SignTool` / `SignedUninstaller=yes`.
3. No packaging-layout change; the two-binary bundle is unaffected.

Until then v1 is correct and shippable; the SmartScreen path above is the
supported install flow and is surfaced in the release notes / README.
