# Transcription Model Management

## Summary

Replace the primary "model file path" UX with a managed Whisper model catalog: users can download, see progress, select installed models by name, and keep a custom `.bin` path as an advanced fallback. The first version optimizes for understandable first use: the app sends the user to model settings to choose and download a model rather than silently downloading large files or assuming a default model.

Requirements clarity score after grill-me: **97/100**.

## Key Changes

- Add a built-in ggml model catalog sourced from `ggerganov/whisper.cpp`: `tiny`, `base`, `small`, `medium`, `large-v3`; include display name, approximate size, short pros/cons, expected filename, download URL, and checksum.
- Extend settings with `paths.models_dir`, `transcriber.model_source` (`managed` or `custom_path`), nullable/absent `transcriber.model_id`, and `transcriber.custom_model_path`; migrate old `transcriber.model_path` to `custom_path`.
- Add authenticated API:
  `GET /api/v1/transcription-models` for catalog/status/current folder,
  `POST /api/v1/transcription-models/:id/install` for checksum-verified download with progress,
  `GET /api/v1/transcription-models/installations/:job_id` for polling progress/errors,
  `DELETE /api/v1/transcription-models/:id` for deleting non-active managed models.
- Download behavior: write to a temporary file, report byte progress, retry transient failures, verify checksum, then atomically rename into `models_dir`. On success, mark the model as installed but do not select it automatically.
- Runtime behavior: resolve the active model path from managed/custom settings on every save and call `LazyWhisperTranscriber::set_model_path(...)`, including resets from custom path back to an explicitly selected managed model.
- Missing/empty model behavior: transcription should fail with `model_not_selected` when no model was chosen, or `model_missing` when the selected file is absent/invalid; no automatic download from the transcription flow.

## UI

- Rework `WhisperPanel.qml` from "Файл модели" to "Модель транскрипции".
- Show model rows/cards with name, size, installed/downloading/missing status, short description, and actions: `Скачать`, `Выбрать`, `Удалить` for non-active installed models.
- Show download progress inline with bytes/percent and a retry action after failure. Cancellation is out of scope for v1.
- Add "Папка моделей" with the current `models_dir` and folder picker. Changing the folder does not move files; the model list simply reflects the new folder contents.
- Keep "Путь к модели" as an advanced option with `.bin` file picker and explanatory text. It stores an external path only; it does not copy or delete that file. Language, beam size, and CPU threads stay in the same panel.

## Test Plan

- Rust unit tests:
  migration from `model_path`;
  managed/custom path resolution;
  no selected model returns `model_not_selected`;
  checksum failure deletes temp file;
  successful install does not change active model;
  deleting active model is rejected;
  custom path deletion is never attempted;
  changing `models_dir` rescans without moving files.
- API contract tests:
  all new routes require auth;
  catalog returns expected public shape;
  install job reports progress and final status;
  missing managed model and no-selected-model produce distinct user-facing errors;
  settings round-trip preserves new fields.
- QML/manual checks:
  first-run `model_not_selected` points to settings;
  download progress renders without layout jumps;
  downloaded model requires explicit `Выбрать`;
  selecting managed and custom models persists;
  switching model unloads the old Whisper runner.

## Assumptions

- New users start with no selected model; the UI asks them to choose and download one before transcription.
- No model is labeled as "recommended"; model descriptions explain speed, quality, and size neutrally.
- v1 includes delete for non-active managed models only.
- Install jobs are in-memory only. Resume downloads and cancellation are out of scope for v1.
- Download source is Hugging Face `ggerganov/whisper.cpp`; `whisper.cpp` documents downloading converted ggml models from that repository.
- Sizes/checksums should be stored as constants and refreshed only when intentionally updating the catalog.

## Grill-Me Locked Decisions

- Use in-app download rather than bundled models or external instructions.
- Do not auto-download during transcription.
- Do not auto-select after download.
- Use separate in-memory install jobs for progress.
- Retry within one job, but do not support resume downloads in v1.
- Keep five catalog models visible without "recommended" wording.
- Add `model_not_selected` separately from `model_missing`.
- Use "Путь к модели" for the custom path UI.
- Delete only non-active managed model files; never delete custom-path files.

## Sources

- [whisper.cpp README](https://github.com/ggml-org/whisper.cpp)
- [ggerganov/whisper.cpp model repository](https://huggingface.co/ggerganov/whisper.cpp/tree/main)
