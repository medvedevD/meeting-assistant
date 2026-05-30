# Transcription Model Management - Implementation Plan

## Summary

Implement managed Whisper model selection so users no longer need to type a `.bin` path for the normal transcription flow. The app will expose a small built-in model catalog, download selected models with progress and checksum verification, let users select installed models by name, and keep a custom model file path as an advanced fallback.

## Phase 1: Model Catalog and Settings

- Add a `TranscriptionModelCatalog` in the Rust app/adapters layer, outside `meeting-core`, with entries for `tiny`, `base`, `small`, `medium`, and `large-v3`.
- Store for each entry: stable id, display name, approximate size, short description, pros/cons, expected `ggml-*.bin` filename, download URL, and checksum.
- Extend persisted settings:
  - `paths.models_dir`, defaulting to the existing app data models directory.
  - `transcriber.model_source`: `managed` or `custom_path`.
  - `transcriber.model_id`, nullable/absent until the user explicitly chooses a model.
  - `transcriber.custom_model_path`.
- Migrate existing `transcriber.model_path` into `transcriber.custom_model_path` and set `model_source = "custom_path"` when present.
- Add a resolver that converts current settings into the effective Whisper model path, returns `model_not_selected` when no model has been chosen, and returns `model_missing` when a selected managed/custom file is absent or invalid.

## Phase 2: Backend API and Download Jobs

- Add authenticated model-management routes:
  - `GET /api/v1/transcription-models`: catalog, install status, selected model, active source, and `models_dir`.
  - `POST /api/v1/transcription-models/:id/install`: start installation for a catalog model.
  - `GET /api/v1/transcription-models/installations/:job_id`: poll byte progress, percent, status, and error.
  - `DELETE /api/v1/transcription-models/:id`: delete installed managed models that are not currently active.
- Implement installer behavior:
  - Create `models_dir` if missing.
  - Download to a temporary file.
  - Report byte progress.
  - Retry transient network failures.
  - Verify checksum before installation.
  - Atomically rename the verified file to its final filename.
  - Remove partial temp files after failure.
- Keep install jobs in memory only; after app/sidecar restart, partial temp files are deleted/ignored and the user can restart the download.
- Keep cancellation and resume downloads out of v1. Retry transient failures within one install job, but start a fresh download after terminal failure.
- Update the sidecar auth route list and contract tests for the new endpoints.

## Phase 3: Runtime Transcriber Wiring

- Update settings hot-apply so every save resolves the effective model path and calls `LazyWhisperTranscriber::set_model_path(...)` when needed.
- Fix the current reset gap: clearing an old model path or switching from custom back to managed must unload the old runner and point the transcriber at the explicitly selected managed model path.
- When transcription starts with no selected model, fail before loading Whisper with `model_not_selected`; when the selected file is missing or broken, use `model_missing`.
- Preserve existing language, beam size, and CPU thread behavior.

## Phase 4: QML Settings UI

- Replace the current `WhisperPanel.qml` "Файл модели" row with a "Модель транскрипции" section.
- Show catalog models as selectable rows/cards with name, size, status, short description, and actions:
  - `Скачать` for missing models.
  - `Выбрать` for installed inactive models.
  - `Удалить` for installed non-active models.
  - Inline progress while downloading.
- On successful download, only mark the model as installed; do not select it automatically. Show `Выбрать` as the explicit next action.
- Add "Папка моделей" with the current folder path and folder picker. Changing it rescans status only; no files are moved.
- Add "Путь к модели" as an advanced custom `.bin` picker. It stores an external path only; it does not copy/import the file into `models_dir`.
- Keep language, beam size, and CPU thread controls in the panel below model management.

## Phase 5: Tests and Verification

- Rust unit tests:
  - settings migration from `model_path`;
  - managed/custom path resolution;
  - no selected model returns `model_not_selected`;
  - missing managed model error;
  - checksum failure cleanup;
  - successful install does not change active model;
  - active model deletion rejected;
  - custom path deletion is never attempted;
  - `models_dir` change rescans without moving files.
- API contract tests:
  - new routes require bearer auth;
  - catalog shape is stable;
  - install job exposes progress and final status;
  - settings round-trip preserves new fields.
- UI/manual verification:
  - first-run `model_not_selected` points to transcription settings;
  - download progress does not jump layout;
  - downloaded model requires an explicit `Выбрать`;
  - managed and custom selection persist after save/reload;
  - switching model unloads the previous Whisper runner.

### Phase 5 Verification Notes - 2026-05-25

- Added regression coverage for Phase 4 follow-up behavior:
  - external custom model paths are not deleted through managed-model deletion;
  - an installed managed model is not selected automatically;
  - changing `models_dir` rescans install status without moving files;
  - settings contract round-trips `models_dir`, `model_source`, `custom_model_path`, and `custom_models`.
- Fixed the sidecar log contract test to force `RUST_LOG=info`; otherwise a caller environment such as `RUST_LOG=warn` can hide the expected readiness log.
- Verified:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml`
  - headless Qt/QML smoke: `env XDG_DATA_HOME=/tmp/ma-qml-check XDG_CONFIG_HOME=/tmp/ma-qml-check XDG_CACHE_HOME=/tmp/ma-qml-check QT_QPA_PLATFORM=offscreen timeout 8s qt-app/build/meeting-assistant-qt`

## Assumptions

- New users start with no selected model; the UI asks them to choose and download a model before transcription.
- Do not label any model as "recommended"; explain trade-offs neutrally through speed, quality, and size.
- v1 supports deletion only for non-active managed models.
- Download source is Hugging Face `ggerganov/whisper.cpp`.
- Sizes/checksums are constants updated only when deliberately refreshing the catalog.
- Planning and PRD artifacts for this task live in `.claude/plans/transcription-model-management/`.

## Grill-Me Locked Decisions

- Use in-app download rather than bundled models or external instructions.
- Do not auto-download during transcription; route the user to settings with an explicit `Скачать` action.
- Do not auto-select after download; installation and active selection are separate user actions.
- Use separate in-memory install jobs for download progress.
- Do not support resume downloads or cancellation in v1.
- Keep all five catalog models visible, with no "recommended" label.
- Add `model_not_selected` separately from `model_missing`.
- "Путь к модели" stores an external path only; it never copies or deletes the file.
- Deleting a managed model physically removes the non-active managed `.bin` from `models_dir`.
