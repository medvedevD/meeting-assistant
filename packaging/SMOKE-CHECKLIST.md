# Release smoke checklist

The acceptance bar for a release: the full happy path completes on macOS (arm64),
Windows (x64), and Linux (x86_64). Run this once per OS from the installed build, not
the dev tree. For any failure, add a regression test before the next tag (see
"Regression Tests" in `RULES.md`).

## Before tagging

- [ ] CI is green on the exact commit to be tagged.
- [ ] `cargo test --manifest-path rust/Cargo.toml` passes locally.
- [ ] `ctest --test-dir qt-app/build` passes locally.
- [ ] `./run-qt.sh` works end to end on one OS. This catches logic and QML bugs without
      the installer round trip.

## Where the logs are

The GUI copies its log, including relayed `[meeting-server]` sidecar lines, to a file.
The previous run is kept as `…log.prev`. Grab this on any failure:

- macOS: `~/Library/Application Support/meeting-assistant/logs/meeting-assistant.log`
- Linux: `~/.local/share/meeting-assistant/logs/meeting-assistant.log`
- Windows: `%LOCALAPPDATA%\meeting-assistant\logs\meeting-assistant.log`

## Install and open

Follow [`docs/INSTALL.md`](../docs/INSTALL.md). On macOS, do not run the app directly
from inside the DMG; see also [`macos/HOMEBREW-SUNSET.md`](macos/HOMEBREW-SUNSET.md). On
Windows, see [`windows/SMARTSCREEN.md`](windows/SMARTSCREEN.md). On Linux, if the
AppImage fails to launch, run it from a terminal to see stderr.

## Happy path

- [ ] The app launches and the window appears.
- [ ] The sidecar reaches Ready, with no blocking "incompatible" or "core failed" dialog.
- [ ] The OS microphone prompt appears and granting it works. On macOS, also check Screen
      Recording if system audio is used.
- [ ] macOS, over an earlier build: the sidecar reaches Ready without waiting for a keychain
      prompt. A recording with system audio fails with the permission dialog; "Reset
      permission" there, then a new start, brings up the macOS prompt, and after granting
      and reopening, the recording starts. Moving the app out of the list and back is not
      enough.
- [ ] Start a recording. The timer runs and the status line names the real device.
- [ ] Stop. The meeting appears in the list.
- [ ] Transcription runs to completion. On Windows or Linux, use a CPU without AVX-512
      at least once, such as a Ryzen 5000 or a 12th-gen Intel Core. CI only proves the
      binary doesn't ask for it.
- [ ] Enter your own API key in Settings. "Test key" succeeds.
- [ ] Generate minutes. They render.
- [ ] Russian minutes are set in one serif: digits and Latin words match the letters
      around them.
- [ ] Minutes in both languages have even letter spacing.
- [ ] On the first-run screen, the text on the tilted card is smooth, not stair-stepped.
- [ ] Play the recording in the meeting card: play, pause, seek, speed, volume.
- [ ] Change a setting, restart the app, and confirm the change persisted.
- [ ] The log file has no errors beyond expected info lines.

## Results

Copy this table into the release notes or your own tracker for each release.

| Step group | macOS arm64 | Windows x64 | Linux x86_64 |
|---|---|---|---|
| Install and launch | ☐ | ☐ | ☐ |
| Sidecar ready | ☐ | ☐ | ☐ |
| Record and stop | ☐ | ☐ | ☐ |
| Transcription | ☐ | ☐ | ☐ |
| Minutes (own key) | ☐ | ☐ | ☐ |
| Audio player | ☐ | ☐ | ☐ |
| Settings persist | ☐ | ☐ | ☐ |
