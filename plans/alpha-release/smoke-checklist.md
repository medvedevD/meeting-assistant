# Alpha smoke checklist (personal, per-OS)

Acceptance bar for `v0.1.0-alpha.1`: the **full happy path** completes on macOS
(arm64), Windows (x64) and Linux (x86_64). Run this once per OS from the
installed build (not the dev tree). Any failure → file a regression test before
the next tag (RULES.md "Regression Tests").

## Before tagging (pre-flight — cheapest catch)

- [ ] CI green on the exact commit to be tagged.
- [ ] `cargo test --manifest-path rust/Cargo.toml` passes locally.
- [ ] `ctest --test-dir qt-app/build` (offscreen Qt smoke) passes.
- [ ] `./run-qt.sh` end-to-end on macOS once — catches logic/QML bugs without the
      installer round-trip.

## Where the logs are

The GUI tees its log (including relayed `[meeting-server]` sidecar lines) to a
file; the previous run is kept as `…log.prev`. Grab this on any failure:

- macOS   `~/Library/Application Support/meeting-assistant/logs/meeting-assistant.log`
- Linux   `~/.local/share/meeting-assistant/logs/meeting-assistant.log`
- Windows `%LOCALAPPDATA%\meeting-assistant\logs\meeting-assistant.log`

## Install + open past the gate

- **macOS** — preferred alpha path: download `install-macos.sh` from the GitHub
  Release, then run `bash ~/Downloads/install-macos.sh`. The release-pinned
  installer downloads and verifies the DMG, installs the app in `/Applications`,
  removes quarantine, verifies the code seal, and launches it. Direct DMG
  fallback: open the `.dmg`, drag to Applications, then run
  `xattr -dr com.apple.quarantine /Applications/MeetingAssistant.app`. This is
  not a public distribution fix; the durable exit is Developer ID signing +
  notarization/stapling. See `packaging/macos/HOMEBREW-SUNSET.md`.
- **Windows** — run `MeetingAssistant-Setup-*.exe`. SmartScreen: **More info →
  Run anyway** (unsigned). See `packaging/windows/SMARTSCREEN.md`.
- **Linux** — `chmod +x MeetingAssistant-*.AppImage` and run it. If it fails to
  launch, run it from a terminal to see stderr.

## Happy path (run on each OS)

- [ ] App launches; window appears.
- [ ] Sidecar reaches **Ready** (no blocking "Incompatible"/"core failed" dialog).
- [ ] Mic permission: the OS prompt appears and granting it works
      (macOS TCC mic; on macOS also screen-recording if system audio is used).
- [ ] Start a recording; the level meter moves.
- [ ] Stop; the meeting appears in the list.
- [ ] Transcription job runs to completion (Whisper model downloads on first use).
- [ ] Enter your own Anthropic API key in Settings; "Test key" succeeds.
- [ ] Generate a protocol; it renders.
- [ ] Play the recording in the meeting card: play/pause, seek, speed, volume.
- [ ] Change a setting (e.g. template/default), restart the app, the change
      persisted.
- [ ] No errors in the log file beyond expected info lines.

## Per-OS results

| Step group | macOS arm64 | Windows x64 | Linux x86_64 |
|---|---|---|---|
| Install + launch | ☐ | ☑ | ☐ |
| Sidecar Ready | ☐ | ☑ | ☐ |
| Record → stop | ☐ | ☑ | ☐ |
| Transcription | ☐ | ☑ | ☐ |
| Protocol (own key) | ☐ | ☑ | ☐ |
| Audio player | ☐ | ☑ | ☐ |
| Settings persist | ☐ | ☑ | ☐ |

Windows x64 verified 2026-06-13 (mic/system/mixed capture, transcription,
protocol with own key, playback, settings persist across restart). Fixes landed this round: audio capture format
+ loopback, real mixed mix + bundled ffmpeg, no-console GUI launch, delete a
recording held open by the player.
