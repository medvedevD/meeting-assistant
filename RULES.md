# RULES.md

This file is the shared source of repository guidance for both Codex and Claude Code. `AGENTS.md` and `CLAUDE.md` must point here so both agents follow the same baseline behavior.

## Project Overview

Meeting Assistant is an AI-powered desktop app for recording, transcribing, and generating meeting protocols. It consists of a Rust backend (clean-architecture core + HTTP sidecar binary) and a Qt 6 (QML) desktop UI that talks to the sidecar over loopback HTTP.

## Planning Artifacts

All plans, requirements, and PRDs go into `.agents/plans/` in this repo — **one folder per task**, with clear nesting (e.g. `.agents/plans/<task-slug>/prd-v1.0.md`). Never use `docs/prds/`, `.Codex/plans/`, `.claude/plans/`, or ad-hoc locations. Tools or skills that generate planning docs must write to this location, overriding their defaults.

## Backlog

Future work that should be tracked but is not yet an active plan goes into
`backlog/` at the repository root. Use one Markdown file per backlog item, named
with a short kebab-case slug (for example,
`backlog/structured-protocol-rendering.md`). Do not use a single monolithic
`BACKLOG.md`; split items into separate files so each task can grow into its own
plan later.

## Development Commands

### Build & Run

```bash
# Full build + run (Rust sidecar → Qt GUI → launch)
./run-qt.sh

# Debug Rust sidecar (faster, skips release optimizations)
./run-qt.sh --debug

# Skip Rust rebuild (sidecar already built)
./run-qt.sh --skip-rust

# Skip Qt rebuild (CMake config + qt-app already built)
./run-qt.sh --skip-build

# Build only, don't launch
./run-qt.sh --no-run
```

### Testing

```bash
# Rust tests (whole workspace)
cargo test --manifest-path rust/Cargo.toml

# Single Rust test
cargo test --manifest-path rust/Cargo.toml -p <crate-name> <test_name>

# Python legacy tests
cd legacy && pytest
```

### Regression Tests

For every bug fix, add or update a regression test at the lowest level that reproduces the observed failure. The test must fail on the old behavior and pass with the fix. If the bug is in QML wiring or screen behavior, prefer a QML integration test over only a C++ unit test; lower-level tests are useful only when they exercise the actual failing contract.

### Git Commit Attribution

When creating commits, do not add AI-agent attribution. Commit messages must not include `Co-authored-by`, `Generated-by`, `Authored-by`, or similar trailers/signatures for Codex, Claude, or any other agent.

### Packaging

See `packaging/` for OS-specific build/sign/notarize scripts (handled outside the dev loop).

## Architecture

### Rust (`/rust`) — Clean Architecture

Four-crate workspace:

| Crate | Role |
|---|---|
| `meeting-core` | Domain entities, port traits, use cases |
| `meeting-adapters` | Concrete implementations (Whisper, audio, SQLite, HTTP) |
| `meeting-api` | REST API via Axum |
| `meeting-assistant` | Binaries: legacy CLI (`meeting-assistant`) + loopback sidecar (`meeting-server`) |

Layers: **Entities → Ports (traits) → Use Cases → Adapters → Binaries**. Use cases depend only on port traits; adapters implement them. Test fakes are behind the `fakes` feature flag.

Key domain flow: `StartRecording` → audio captured via `AudioCapture` port → `StopRecording` → async job queued → `TranscribeAudio` (Whisper) → `GenerateProtocol` (Anthropic API via `LLMProvider` port).

**Database**: SQLite via rusqlite, migrations in `/rust/migrations/`.

### Qt UI (`/qt-app`) — QML + C++ shell

- `src/main.cpp` — Qt entry point; bootstraps the QML engine and singletons.
- `src/SidecarManager.{h,cpp}` — locates and launches the `meeting-server` binary next to the GUI executable, manages its lifecycle.
- `src/ApiClient.{h,cpp}` — typed wrapper over the loopback HTTP API.
- `src/JobPoller.{h,cpp}` — polls async job state (transcription / protocol generation).
- `qml/Main.qml`, `qml/AppShell.qml`, `qml/MeetingStore.qml`, `qml/screens/` — UI tree.

### Sidecar Bridge

`run-qt.sh` orchestrates: `cargo build --bin meeting-server` → configure & build the Qt app via CMake → copy the sidecar binary next to the Qt executable (so `SidecarManager` finds it via `applicationDirPath()`) → launch the GUI. The GUI spawns the sidecar on startup and talks to it on a loopback port.

### Prompts

LLM prompt templates live in `/prompts/` (Russian language). The `TemplateLoader` port reads these at runtime.

### Settings persistence and hot-swap

Settings and secrets deliberately live **outside** the clean-architecture core — there are no `SettingsStore`/`SecretStore` ports. They are adapters wired in the composition root and consumed only there and in the API layer.

- **Persistence**: `JsonSettingsStore` ([settings_store.rs](rust/crates/adapters/src/settings_store.rs)) writes `PersistedSettings` to `settings.json`. Secrets never touch that file — `KeyringSecretStore` ([secret_store.rs](rust/crates/adapters/src/secret_store.rs)) stores per-provider API keys in the OS keyring, falling back to a `0600` JSON file (`~/.config/meeting-assistant/secrets.json`) when the keyring is unavailable (e.g. an unsigned macOS dev build). `MEETING_ASSISTANT_KEYRING_DISABLE=1` forces the file fallback (used by tests). Env vars (`ANTHROPIC_API_KEY`, etc.) always override stored keys via `effective_key`.
- **HTTP surface**: `GET/PUT /api/v1/settings` (sanitized — secrets become `has_key: bool`, never values), `PUT /api/v1/settings/secret`, `POST /api/v1/settings/test`. Wired by `AppSettingsService` ([app/src/settings_service.rs](rust/crates/app/src/settings_service.rs)), which implements the `SettingsService` trait the `meeting-api` crate depends on.
- **Hot-swap on `PUT`**: the active LLM is swapped via `SwappableLlm` (`ArcSwap<dyn LlmProvider>`); transcriber prefs/model and the prompts dir are applied in place. Only `db_path` and `recordings_dir` changes are **restart-required** (long-lived `Db`/repos/worker hold them) — surface a restart banner rather than trying to apply them live.
- **`default_template` resolution (decision #3)**: the core `generate_protocol` use-case only ever receives a ready `template_name`. When a request omits one, the **API layer** resolves the configured default via the `AppState.default_template` resolver (`DefaultTemplateFn`, a closure over the live settings store) before calling the use-case — both in `routes/protocols.rs` and `routes/meetings.rs` (`reprocess` enqueues the resolved name). This keeps "settings" from leaking into the core. Tests and the legacy CLI use `no_default_template()`.

### Adding a new LLM provider

The `LlmProvider` port is intentionally minimal (`generate(transcript, instructions) -> String`). To add a provider:

1. **`ProviderKind`** ([llm/config.rs](rust/crates/adapters/src/llm/config.rs)) — add the variant plus its `as_str`/`parse` aliases, `default_model`, `default_base_url`, and `needs_key`.
2. **Adapter** — if it speaks the OpenAI Chat Completions wire format, reuse `OpenAiCompatProvider` (as OpenAI/Mistral/Ollama do); otherwise add a new file modeled on `anthropic.rs`/`gemini.rs` with a `generate` and a cheap `probe` (the "Test key" button). Map HTTP errors through `errors.rs` (`classify_http`/`classify_transport`) so the pipeline error classes stay consistent.
3. **Factory** ([llm/factory.rs](rust/crates/adapters/src/llm/factory.rs)) — wire `build_llm` and `probe_llm` for the new kind.
4. **Settings** — add a `ProviderCfg` field on `LlmPrefs` in [settings_store.rs](rust/crates/adapters/src/settings_store.rs) and expose it in the `snapshot()`/`provider_view` in [app/src/settings_service.rs](rust/crates/app/src/settings_service.rs). Add the env-var mapping in `secret_store.rs` if it authenticates with a key.
5. **Tests** — add a `wiremock` test for the new wire path (see existing tests in `llm/`).

### Voice Activity Detection (VAD) — not implemented (decided Phase 6)

VAD is intentionally **not** wired into the Whisper transcriber. `whisper-rs` VAD requires bundling and managing a **separate silero model file**, which adds packaging/notarization surface (an extra signed asset, download/version management) without changing transcription correctness — Whisper's own decoding handles silence acceptably. Given the shipping focus (signing/notarization/updater), the cost outweighs the benefit for now. Revisit only if real recordings show meaningful wasted compute on long silences; if added, plan it as its own task (model acquisition + settings UI for `vad`/`vad_threshold` + transcriber wiring).

## Environment

`ANTHROPIC_API_KEY` must be set for protocol generation. The app shows a warning on startup if missing.

## Key Files

- [run-qt.sh](run-qt.sh) — main dev workflow script
- [rust/Cargo.toml](rust/Cargo.toml) — workspace root
- [rust/crates/core/src/lib.rs](rust/crates/core/src/lib.rs) — domain layer entry
- [rust/crates/app/src/bin/meeting-server.rs](rust/crates/app/src/bin/meeting-server.rs) — loopback HTTP sidecar binary
- [qt-app/CMakeLists.txt](qt-app/CMakeLists.txt) — Qt build entry
- [qt-app/src/main.cpp](qt-app/src/main.cpp) — Qt entry point
- [qt-app/qml/Main.qml](qt-app/qml/Main.qml) — root QML
- [rust/migrations/](rust/migrations/) — SQLite schema history
