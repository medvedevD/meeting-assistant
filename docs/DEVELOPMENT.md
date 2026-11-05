# Development

See the [architecture overview](ARCHITECTURE.md) for how the parts fit together, and
[`RULES.md`](../RULES.md) for project conventions.

## Prerequisites

- Rust. The version is pinned in `rust-toolchain.toml`, and rustup installs it
  automatically.
- CMake 3.21 or newer.
- Qt 6.7 or newer with Quick, Quick Controls 2, Network, Multimedia, Svg, and Linguist tools.
- ffmpeg, which mixes microphone and system audio.

Platform notes:

- **macOS:** `brew bundle` installs CMake, pkg-config, and ffmpeg from the `Brewfile`.
- **Linux (Debian or Ubuntu):** these are the packages CI uses for the Rust gate. System
  audio also needs `pactl` and `parec` at runtime.

  ```bash
  sudo apt-get install clang libclang-dev libasound2-dev pkg-config libssl-dev libdbus-1-dev
  ```

- **Windows:** use MSVC and `run-qt.ps1`, which has the same options as `run-qt.sh`.

## Build and run

```bash
./run-qt.sh
```

This builds the `meeting-server` sidecar and the Qt app, copies the sidecar next to
the GUI executable, and launches the app.

| Option | Effect |
|---|---|
| `--debug` | Debug build of the sidecar, faster to compile |
| `--skip-rust` | Don't rebuild the sidecar |
| `--skip-build` | Don't reconfigure or rebuild the Qt app |
| `--no-run` | Build only |
| `--cuda`, `--vulkan` | Build the sidecar with a GPU Whisper backend (needs the SDK) |

To run as a clean first install, with temporary data and config folders that leave
your real meetings and settings untouched:

```bash
FIRST_RUN=1 ./run-qt.sh
```

## Test

```bash
cargo test --manifest-path rust/Cargo.toml
```

```bash
ctest --test-dir qt-app/build
```

CI also runs these checks, and they gate every release:

```bash
.github/scripts/check-protocol-version.sh
```

```bash
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

```bash
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
```

```bash
.github/scripts/check-translations.sh
```

## Translations

Russian is the source language of the interface strings, and
`qt-app/i18n/meeting-assistant_en.ts` holds the English translation. After changing a
string, refresh the file and translate what it marks as unfinished:

```bash
cmake --build qt-app/build --target update_translations
```

## Developer tools

- `rust/scripts/` has helpers for recording from the command line, benchmarking
  transcription, and measuring the macOS clock drift between microphone and system
  audio.
- `meeting-server --rollback-one` rolls back the latest migration when it includes
  `down` SQL.

## Releases

Pushing a `v*` tag runs `.github/workflows/release.yml`, which publishes a prerelease
with all three installers. Run the [smoke checklist](../packaging/SMOKE-CHECKLIST.md)
on the installed builds. Packaging recipes are described in
[`packaging/README.md`](../packaging/README.md).
