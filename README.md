<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/logo-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="docs/brand/logo-light.svg">
    <img alt="Meeting Assistant" src="docs/brand/logo-light.svg" width="520">
  </picture>
</h1>

[![CI](https://github.com/medvedevD/meeting-assistant/actions/workflows/qt-ci.yml/badge.svg?branch=main)](https://github.com/medvedevD/meeting-assistant/actions/workflows/qt-ci.yml)
[![Release](https://img.shields.io/github/v/release/medvedevD/meeting-assistant?include_prereleases&label=release)](https://github.com/medvedevD/meeting-assistant/releases)
[![License: MIT](https://img.shields.io/github/license/medvedevD/meeting-assistant)](LICENSE)

**English** | [Русский](README.ru.md)

Record a meeting, get a transcript and structured minutes. Speech recognition runs on
your computer, and you choose which AI writes the minutes.

> **Alpha.** Builds are not yet signed with commercial certificates.

## Features

- **Record any source:** microphone, system audio, or both, or import a file.
- **Transcribe privately:** Whisper runs locally and audio never leaves your computer.
- **Minutes from templates:** stand-ups, 1-on-1s, team meetings, all editable.
- **Your choice of AI:** Anthropic, OpenAI, Gemini, or Mistral with your own key, or Ollama offline.
- **Meeting history:** keep recordings, transcripts, and minutes locally; replay or reprocess any meeting.

## Download

Get the latest build for macOS, Windows, or Linux from
[Releases](https://github.com/medvedevD/meeting-assistant/releases). On macOS you can
also use Homebrew:

```bash
brew tap medvedevd/meeting-assistant && brew install --cask meeting-assistant
```

The [installation guide](docs/INSTALL.md) covers first-launch warnings and setup.

## Privacy

Audio is processed only on your computer. The transcript goes to the LLM provider you
selected, and only when you generate minutes. With Ollama, nothing leaves your computer.
API keys are kept in your OS keychain when one is available.

## How it's built

**Stack:** Rust, Qt 6 (QML and C++), whisper.cpp, SQLite, Axum.

- **Two processes:** the Qt UI starts a Rust sidecar and talks to it over loopback HTTP with a per-launch token, so a crash in native code does not take down the UI.
- **Clean architecture:** domain logic depends only on interfaces, and Whisper, audio, storage, and LLM providers plug in as adapters.
- **Durable jobs:** a queue in SQLite saves each result before a job finishes, so a retry never repeats Whisper or a paid LLM request.
- **Native audio on each OS:** ScreenCaptureKit on macOS, WASAPI loopback on Windows, and PulseAudio/PipeWire on Linux.
- **Gated releases:** a version tag builds installers only after the protocol check, formatting, Clippy, and tests pass.

Read more in the [architecture overview](docs/ARCHITECTURE.md). To build and run it yourself, see the [development guide](docs/DEVELOPMENT.md).

## Repository map

| Path | Contents |
|---|---|
| `rust/` | Core, adapters, HTTP API, and the sidecar |
| `qt-app/` | Desktop UI in QML and C++, with its tests |
| `prompts/` | Built-in minutes templates |
| `packaging/` | Installer recipes, license notices, and the release checklist |
| `docs/` | Installation, development, architecture, and brand assets |
| `plans/`, `backlog/` | Work in progress and future ideas |

## License

[MIT](LICENSE). Packaged builds also include Qt and FFmpeg under the LGPL; see
[`packaging/LICENSES`](packaging/LICENSES).
