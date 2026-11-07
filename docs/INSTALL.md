# Installation

**English** | [Русский](INSTALL.ru.md)

Prebuilt packages are on
[GitHub Releases](https://github.com/medvedevD/meeting-assistant/releases), for macOS
on Apple silicon, Windows x64, and Linux x86_64. They are alpha builds, not yet signed
with commercial Apple or Microsoft certificates, so macOS and Windows show a warning
on first launch.

The Windows and Linux builds need a CPU with AVX2: Intel Haswell (2013) or AMD Excavator
(2015) and later. Some Pentium, Celeron and Atom chips don't have it. On such a CPU the
app opens, but transcription fails.

## macOS

Recommended, with Homebrew:

```bash
brew tap medvedevd/meeting-assistant
```

```bash
brew install --cask meeting-assistant
```

If Homebrew asks you to trust the third-party tap, run the `brew trust` command it
shows, then repeat the installation.

To update:

```bash
brew upgrade --cask meeting-assistant
```

You can also download `MeetingAssistant-*.dmg` from Releases:

1. Open the DMG and drag MeetingAssistant to Applications.
2. Launch the app from Applications.
3. If macOS blocks the first launch, open **System Settings → Privacy & Security** and
   click **Open Anyway**.

The DMG is ad-hoc signed but not notarized by Apple. The warning can only be removed
completely by moving to a Developer ID and notarization.

## Windows

Download `MeetingAssistant-Setup-*.exe` from Releases and run the installer. Because
the build is not signed, SmartScreen may require you to choose
**More info → Run anyway**.

## Linux

Download the AppImage, make it executable, and run it:

```bash
chmod +x MeetingAssistant-*.AppImage
```

```bash
./MeetingAssistant-*.AppImage
```

## First run

1. Allow the app to access the microphone. On macOS, recording system audio also
   requires the Screen Recording permission.
2. Select or add a Whisper model in the settings.
3. Configure an LLM provider. Cloud providers require your own API key, and Ollama runs
   locally without a key.
4. Create a recording, wait for the transcription to finish, and generate the minutes.

## Logs

If something goes wrong, the application log is here:

- **macOS:** `~/Library/Application Support/meeting-assistant/logs/meeting-assistant.log`
- **Linux:** `~/.local/share/meeting-assistant/logs/meeting-assistant.log`
- **Windows:** `%LOCALAPPDATA%\meeting-assistant\logs\meeting-assistant.log`
