# Portfolio readiness

## Problem

The repository is linked from the author's LinkedIn profile, but a visitor cannot
quickly see what the app does or judge its quality. The README has no visuals and
is mostly install steps, there is no `LICENSE`, and finished process files sit in
`plans/`. The app itself is Russian-only and shows two names: "meety" in the UI,
"Meeting Assistant" everywhere else.

**Readers:** recruiters and engineers first, possible users second. The README
leads with the product and follows with the engineering.

## Scope

**In**

- Repository cleanup without moving code directories.
- New short README (EN and RU), with details moved to `docs/`.
- MIT `LICENSE`, `docs/ARCHITECTURE.md`, GitHub topics and badges.
- One visible name in the app: "Meeting Assistant".
- RU and EN app: UI, processing stages, errors, bundled templates, generated minutes.
- Real screenshots and a GIF using fictional data.
- A new alpha release, so the English build can actually be downloaded.

**Out**

- Moving or renaming `rust/`, `qt-app/`, `packaging/`, or crates.
- Renaming internal `Meety*` components and theme identifiers.
- A third UI language, or translating the bundled templates of existing users.
- Apple Developer ID, notarization, Windows code signing.
- New product features, including transcript search.
- RU versions of `ARCHITECTURE.md` and `DEVELOPMENT.md`.
- The LinkedIn project text (not selected).

## Draft copy

Final wording ships in the READMEs. Every claim below is already true of the app.

**EN**

> Record a meeting, get a transcript and structured minutes. Speech recognition
> runs on your computer, and you choose which AI writes the minutes.
>
> - **Record any source:** microphone, system audio, or both, or import a file.
> - **Transcribe privately:** Whisper runs locally and audio never leaves your computer.
> - **Minutes from templates:** stand-ups, 1-on-1s, team meetings, all editable.
> - **Your choice of AI:** Anthropic, OpenAI, Gemini, or Mistral with your own key, or Ollama offline.
> - **Meeting history:** keep recordings, transcripts, and minutes locally; replay or reprocess any meeting.

**RU**

> Запишите встречу и получите расшифровку и готовый протокол. Речь распознаётся
> прямо на вашем компьютере, а протокол пишет выбранная вами нейросеть.
>
> - **Любой источник:** микрофон, системный звук или оба сразу; можно загрузить готовый файл.
> - **Расшифровка без облака:** Whisper работает локально, аудио не покидает компьютер.
> - **Протоколы по шаблонам:** дейлики, встречи 1-на-1, командные встречи; шаблоны можно менять.
> - **Любая нейросеть:** Anthropic, OpenAI, Gemini или Mistral с вашим ключом либо Ollama офлайн.
> - **История встреч:** записи, расшифровки и протоколы хранятся локально; встречу можно переслушать или обработать заново.

## Deliverables

### Phase 1: repository (PR 1, no app behaviour change, size S)

Ships first because it improves what visitors see immediately and carries no
release risk.

| File | Change |
|---|---|
| `LICENSE` | New: MIT, © 2026 Dmitry Medvedev. |
| `rust/Cargo.toml`, crate manifests | Add `license = "MIT"`. |
| `README.md`, `README.ru.md` | Rewrite, same sections in both: logo and badges, one-sentence pitch, visual, features, download link, privacy note, "How it's built" (5 lines plus link), repository map, license. |
| `docs/INSTALL.md`, `docs/INSTALL.ru.md` | New: install, first run, log paths (moved from README). |
| `docs/DEVELOPMENT.md` | New: build, run, test commands (moved from README). |
| `docs/ARCHITECTURE.md` | New: Mermaid diagrams (GUI and sidecar, crate layers, processing pipeline) and key decisions with reasons: sidecar over FFI, clean architecture, result persistence invariant, secrets in the OS keyring, GPU support, no VAD. |
| `plans/*` | Audit below. Delete 3 empty local folders. |
| `backlog/*` | New: unfinished items from the removed plans. |
| `packaging/SMOKE-CHECKLIST.md` | New: release checklist, generalized from `plans/alpha-release/smoke-checklist.md`. The stale per-OS results are dropped. |
| `run-drift-spike.sh` | Move to `rust/scripts/drift-spike.sh`, fix its relative paths. `run-qt.sh` and `run-qt.ps1` stay in the root as the main entry points. |
| `RULES.md` | Remove the nonexistent `legacy/` pytest command, correct the env-var and job-tracking statements, link the new docs. |
| Code comments | Remove or redirect comments pointing at plans and ADRs that no longer exist. Comments only, no code changes. |
| GitHub settings | Add topics with `gh repo edit`: `rust`, `qt6`, `qml`, `whisper`, `llm`, `speech-to-text`, `meeting-notes`, `desktop-app`. |

Plan audit, confirmed against the code. Durable decisions moved into
`docs/ARCHITECTURE.md` or `RULES.md`, as `RULES.md` requires.

| Plan | Result |
|---|---|
| `audio-source-selection` | Shipped: delete. Unfinished items go to `backlog/recording-level-meter.md` and `backlog/audio-device-hot-change.md`. |
| `secret-storage` | Shipped, including the vault and migrations: fold decisions, delete. |
| `whisper-gpu-acceleration` | Shipped: fold decisions, delete. The proposed Vulkan build goes to `backlog/whisper-gpu-release-build.md`. |
| `transcript-format-modernization` | Shipped (`render_markdown` is used by the worker): delete. |
| `alpha-release` | Goal met: keep the checklist in `packaging/`, delete the rest. |
| `reversible-migrations` | Shipped (`down` support and `--rollback-one` exist; older migrations stay forward-only by design): fold the decision, delete. |

### Phase 2: app localization and name (PR 2, size L)

**Name**

- Replace visible "meety" in `AppShell.qml`, `WelcomeScreen.qml`,
  `NewRecordingScreen.qml`, `LlmPanel.qml`, `StoragePanel.qml`, `WhisperPanel.qml`.
- Adjust the wordmark layout, since the longer name may not fit the sidebar.

**Qt**

| File | Change |
|---|---|
| `qt-app/src/Language.{h,cpp}` | New: resolve the language, store the choice in `QSettings`, install `QTranslator`, set `QLocale::setDefault` so dates and numbers match. Runs before the QML engine loads. |
| `qt-app/CMakeLists.txt` | Add `LinguistTools` and `qt_add_translations`; compiled translations are embedded in the binary. |
| `qt-app/i18n/meeting-assistant_en.ts` | New: English translation of all strings. |
| `qt-app/qml/i18n/errors.js` | Wrap the 21 hardcoded titles and hints in `qsTr`. |
| `qt-app/qml/Main.qml` and other QML | Wrap the remaining unwrapped user-visible strings. |
| `qt-app/qml/components/PipelineProgress.qml` | Use the existing stage-to-label mapping instead of the backend `sub` text. |
| `qt-app/qml/components/ProtocolDocument.qml` | Add English "empty section" phrases. |
| `qt-app/qml/screens/settings/` | Language selector (System, Русский, English), reusing the restart banner. |
| `qt-app/src/SidecarManager.cpp` | Pass the resolved language to the sidecar at launch. |

**Rust**

| File | Change |
|---|---|
| `prompts/ru/`, `prompts/en/` | Move the 4 current templates to `ru/`; add 4 English ones. |
| `adapters/src/template_bundle.rs` | Tag bundled templates by language; `backfill_templates` seeds only the active language. |
| `app/src/bin/meeting-server.rs` | Read the language at launch. When absent, use `ru`, so the CLI keeps its behaviour. |
| `core/src/usecases/generate_protocol.rs` | Make the fallback prompt language-neutral: write in the transcript's language. |
| `core/src/usecases/transcript_render.rs` | Remove the fixed Russian words from the `transcript.md` header. |
| `adapters/src/worker.rs` | Replace Russian stage labels; the GUI now localizes them. |
| `adapters/src/whisper.rs`, `core/src/entities/job.rs` | English error text. The classifier also keeps recognizing the old Russian text. |
| `app/src/template_service.rs` | Return notice codes that the GUI localizes. Check whether this needs a protocol version bump. |
| `core/src/usecases/import_audio.rs` | English fallback name. |
| `core/src/entities/protocol.rs` | Add English section headings used by the new templates. |
| `adapters/src/settings_store.rs` | New installs default to Whisper language `auto`. |

**Other**

- `packaging/macos/make-dmg-background.swift`: English text, regenerate the image.
- `.github/scripts/check-translations.sh` and `qt-ci.yml`: fail on unfinished or
  outdated translations.
- `RULES.md`: add an i18n section, update the Prompts section.
- `README.*`: remove "the interface is in Russian".

### Phase 3: visuals and release (PR 3, size S, depends on phase 2)

- Isolated demo profile with a temporary home directory and fictional meetings,
  so no real recordings, transcripts, or API keys can appear in images.
- `docs/screenshots/`: a GIF under 5 MB and 3 PNGs (recording, progress, minutes),
  from the English UI on macOS. Both READMEs share them.
- Add the visuals to the first screen of both READMEs.
- Release `v0.1.0-alpha.14`. **The author confirms before the tag is pushed.**
- Run `packaging/SMOKE-CHECKLIST.md` from the installed build on each OS. Windows
  and Linux need the author's machines.

## Test plan

**Rust**

- Fresh EN install gets only English templates.
- Existing RU install is unchanged after the upgrade: no new templates, edits
  preserved, deleted bundled templates stay deleted.
- `classify_section` handles every heading in the English templates.
- The error classifier accepts both the new English and the old Russian text.
- The fallback prompt contains no fixed language instruction.

**Qt**

- Language resolution table: `ru_RU` → ru, `en_US` → en, `de_DE` → en, and an
  explicit choice overrides the system.
- `errors.js` returns English with the EN translator loaded.
- `PipelineProgress` shows the localized label when the backend sends Russian `sub`.
- Existing QML tests keep their Russian assertions: they load no translator, so
  Russian source strings remain valid.

**Gates and manual checks**

- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `ctest`, and
  translation check all pass on the 3 CI platforms.
- Per the project's release approach, the manual cross-OS smoke run is the
  integration test.

## Acceptance criteria

- [ ] First screen of each README shows the name, pitch, visual, and features.
- [ ] Each README is at most 80 lines, with matching sections in EN and RU.
- [ ] GitHub detects the MIT license; topics and badges are visible.
- [ ] All links in the READMEs and `docs/` resolve.
- [ ] `plans/` has no finished plans; the repository root has no spike scripts.
- [ ] No user-visible "meety" remains in the app.
- [ ] System language other than Russian: the app opens in English.
- [ ] System language Russian: the app opens in Russian.
- [ ] The selector switches language after a restart.
- [ ] English UI: no Cyrillic on any screen, and the English translation has 0 unfinished strings.
- [ ] English template on an English transcript gives English minutes with correctly recognized sections.
- [ ] Upgrading an existing Russian profile keeps its templates and settings.
- [ ] Images contain no real names, meetings, or keys.
- [ ] `v0.1.0-alpha.14` installs and passes the smoke checklist on all 3 OSes.

## Decisions

1. **Russian stays the source language in `qsTr`.** Qt convention prefers English,
   but switching rewrites about 475 strings and 15 test files with no benefit to
   users. Only `meeting-assistant_en.ts` is added.
2. **English is the fallback for any non-Russian system.** The translation is loaded
   unless the resolved language is Russian.
3. **The language lives in `QSettings`, not `settings.json`.** The GUI needs it
   before the sidecar is running, for example for startup error dialogs.
4. **Changing the language applies after a restart.** It reuses the existing restart
   banner and gives the sidecar one fixed language per run. Most readers get the
   right language automatically, so live switching is not worth a second code path.
5. **Template language determines output language.** Templates are ordinary editable
   files. An English template on a Russian meeting gives English minutes.
6. **Whisper defaults to `auto` for new installs.** A single default works for both
   languages. Saved settings keep their current value.
7. **Backend text becomes language-neutral where possible.** Stage labels and notices
   are localized in the GUI, so the sidecar needs the language only to seed templates.
8. **DMG background is English only.** The image is static and shared by all users.
9. **Helper script goes to `rust/scripts/`.** That folder already exists, so no new
   top-level `scripts/` is needed.

## Risks

- **`LinguistTools` availability:** must be present in all 3 CI Qt installs, including
  the aqt install in the Linux container. The first CI run of PR 2 verifies this.
- **`qsTr` in `.pragma library`:** `errors.js` is a JS library. If lookups fail there,
  pass translated strings in from QML instead. A test covers this.
- **Auto-detected language:** `auto` can misdetect short or mixed-language audio. The
  picker in settings stays as the override.
- **GIF capture:** needs the app driven on macOS with Screen Recording permission,
  and may need the author's help.
- **Smoke run:** Windows and Linux can only be checked on the author's machines, which
  blocks the final acceptance criterion but not phases 1 and 2.

---

Clarification rounds: 2. Clarity score: 92/100.
