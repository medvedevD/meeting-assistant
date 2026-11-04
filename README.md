<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/logo-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="docs/brand/logo-light.svg">
    <img alt="Meeting Assistant" src="docs/brand/logo-light.svg" width="520">
  </picture>
</h1>

**English** | [Русский](README.ru.md)

Meeting Assistant is a desktop app for macOS, Windows, and Linux that records
meetings, transcribes speech locally with Whisper, and generates meeting minutes
with an LLM. It is built with Rust (core and local HTTP sidecar) and Qt 6 with
QML and C++ (desktop UI).

> This project is in alpha. Builds are intended for testing and are not yet
> signed with commercial Apple or Microsoft certificates.

## Features

- Record the microphone, system audio, or a mix of both.
- Transcribe locally with Whisper.
- Generate minutes from editable templates.
- Use Anthropic, OpenAI, Gemini, Mistral, or local Ollama as the LLM provider.
- Browse meeting history, reprocess meetings, and play recordings in the app.
- Keep data and settings on your own computer.

The interface and built-in templates are currently in Russian.

## Installation

Prebuilt packages are available on
[GitHub Releases](https://github.com/medvedevD/meeting-assistant/releases).

### macOS

Recommended free channel:

```bash
brew tap medvedevd/meeting-assistant
brew install --cask meeting-assistant
```

If Homebrew asks you to trust the third-party tap, run the `brew trust` command
it shows, then repeat the installation.

To update:

```bash
brew upgrade --cask meeting-assistant
```

You can also download `MeetingAssistant-*.dmg` from Releases:

1. Open the DMG and drag MeetingAssistant to Applications.
2. Launch the app from Applications.
3. If macOS blocks the first launch, open
   **System Settings → Privacy & Security** and click **Open Anyway**.

The DMG is ad-hoc signed but not yet notarized by Apple. The Gatekeeper warning
can only be removed completely after moving to a Developer ID and notarization.

### Windows

Download `MeetingAssistant-Setup-*.exe` from Releases and run the installer.
Because the alpha build is not yet signed, SmartScreen may require you to choose
**More info → Run anyway**.

### Linux

Download the AppImage and make it executable:

```bash
chmod +x MeetingAssistant-*.AppImage
./MeetingAssistant-*.AppImage
```

## First run

1. Allow the app to access the microphone. On macOS, recording system audio also
   requires the Screen Recording permission.
2. Select or add a Whisper model in the settings.
3. Configure an LLM provider. Cloud providers require your own API key; Ollama
   runs locally without a key.
4. Create a recording, wait for the transcription to finish, and generate the
   minutes.

Whisper processes audio locally, and audio is not sent to the cloud. Transcript
text is sent out only to the LLM provider you selected, and only when generating
minutes.

## Development

Requires Rust, CMake, and Qt 6.7+ with the Quick, Network, Multimedia, and Svg
modules.

Main dev workflow:

```bash
./run-qt.sh
```

Useful variants:

```bash
./run-qt.sh --debug
./run-qt.sh --no-run
FIRST_RUN=1 ./run-qt.sh
```

Tests:

```bash
cargo test --manifest-path rust/Cargo.toml
ctest --test-dir qt-app/build
```

## Architecture

- `rust/`: Rust core, adapters, HTTP API, and the `meeting-server` sidecar.
- `qt-app/`: Qt 6 desktop UI in QML and C++.
- `prompts/`: built-in minutes templates.
- `packaging/`: builds installation artifacts for macOS, Linux, and Windows.

The GUI launches the sidecar from next to its own executable and talks to it over
loopback HTTP, using a bearer token generated at each launch.

## Diagnostics

Application logs:

- macOS: `~/Library/Application Support/meeting-assistant/logs/meeting-assistant.log`
- Linux: `~/.local/share/meeting-assistant/logs/meeting-assistant.log`
- Windows: `%LOCALAPPDATA%\meeting-assistant\logs\meeting-assistant.log`
