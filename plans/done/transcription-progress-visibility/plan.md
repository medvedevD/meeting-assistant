# Plan: Transcription Progress Visibility

> Источник: [backlog/transcription-progress-visibility.md](../../../backlog/transcription-progress-visibility.md).

## Context

После остановки длинной записи (например, часовой встречи) пользователь видит индикатор обработки **без процента готовности**. Причина: `GenerateProtocolScreen` для шага транскрипции ходит в синхронный `POST /api/v1/transcribe`, который блокирует запрос до конца расшифровки и не отдаёт прогресс. В UI крутится indeterminate‑полоска без процента и без подстадии.

При этом транскрибирующий воркер уже умеет писать прогресс (`loading_model / decoding_audio / transcribing / writing_transcript`, percent 0–100) в общую `LiveProgress` DashMap, а `PipelineProgress.qml` уже умеет это рисовать — этим механизмом пользуется только шаг «Протокол». Эндпоинт `POST /api/v1/meetings/:id/reprocess {kind:"transcribe"}` ставит транскрибирующий job и возвращает `job_id`, который можно поллить.

Цель: показать процент и подстадию транскрипции тем же `PipelineProgress`, что используется для шага «Протокол».

## Подход (выбранный вариант)

**Фронт сам цепочит два job‑а:** сначала `kind:"transcribe"`, по успеху — `kind:"protocol"`. Один и тот же `PipelineProgress` на экране, jobId меняется при переходе. Бэкенд не трогаем (никакого `kind:"transcribe_and_protocol"`).

**`POST /api/v1/transcribe` (sync) удаляем полностью** — роут и его юнит‑тест. CLI не затрагивается, он ходит в core напрямую.

**Единое `failed`‑состояние** — отдельный `transcriptFailed` уходит, ошибку любой стадии рендерит `PipelineProgress`.

## Файлы и изменения

### Qt UI

1. **[qt-app/qml/screens/GenerateProtocolScreen.qml](../../../qt-app/qml/screens/GenerateProtocolScreen.qml)**
   - State machine `st`: убрать `"transcribing"` и `"transcriptFailed"`. Остаются `"idle" | "running" | "done" | "failed"`. (`"generating"` переименовать в `"running"`.)
   - Заменить ветку `if (hasTranscript) startProtocolJob() else POST /api/v1/transcribe ...` на:
     - `if (hasTranscript)` → enqueue protocol‑job (как сейчас).
     - Иначе → enqueue transcribe‑job через `POST /api/v1/meetings/:id/reprocess {kind:"transcribe"}`, выставить `scr.jobId` в результат.
   - Удалить `transcribeReq` (Request) и его обработчики.
   - Сохранить локальное состояние «текущая фаза» (`phase: "transcribe" | "protocol"`), чтобы `onProtocolJobFinished` знал, что делать:
     - `phase="transcribe"` + `status="done"` → enqueue protocol‑job, переключить `phase="protocol"`, обновить `scr.jobId` новым.
     - `phase="protocol"` + `status="done"` → showDetail (как сейчас).
     - Любой `status="failed"` → `st="failed"`, `errorMsg = job.last_error || "..."`.
   - Объединить визуальные блоки `transcribing` и `generating | done` в один — теперь оба идут через `PipelineProgress`.
   - Кнопка «Назад» включена при `st in ("idle","failed")`.
   - Pill в шапке: вместо двух состояний `transcribing`/`generating` показывать стадию из текущего job‑а (источник истины — `PipelineProgress.statusLabel()` или собственный mapper). Можно убрать локальный `stateLabel()` и проставить надписи через свойство, проброшенное наружу из `PipelineProgress`.

2. **[qt-app/qml/components/PipelineProgress.qml](../../../qt-app/qml/components/PipelineProgress.qml)** — изменений по логике, скорее всего, не требуется. Component уже:
   - реагирует на `onJobIdChanged: start()` (строка 124) — корректно перезапустит поллер при свопе jobId с transcribe→protocol;
   - сбрасывает `terminalEmitted` в `start()` — не «съест» второй `finished` для protocol‑job.
   - **Проверить:** при свопе jobId сразу после `done` транскрипции есть один тик, когда `job.status === "done"` от прошлого job‑а ещё «висит» и `done`‑ветка `displayPercent=100` остаётся видимой. Возможно, нужно сбрасывать `root.job = {}` в `start()`. Это маленькое уточнение делается при разработке.

### Rust API

3. **[rust/crates/api/src/router.rs](../../../rust/crates/api/src/router.rs)**
   - Убрать `.route("/api/v1/transcribe", post(transcribe::handle))` (строка ~140).
   - Убрать из теста контракта `("POST", "/api/v1/transcribe")` (строка ~351).

4. **[rust/crates/api/src/routes/transcribe.rs](../../../rust/crates/api/src/routes/transcribe.rs)**
   - Удалить файл целиком (включая `writes_transcript_md_next_to_audio_and_records_path` тест — поведение «положить transcript.md рядом с audio» уже покрыто воркером через тот же `file_store.write_transcript`).

5. **`rust/crates/api/src/routes/mod.rs`** (или `routes.rs`) — убрать `pub mod transcribe;`.

### Тесты

6. **Rust integration test** (новый) — `rust/crates/api/src/routes/meetings.rs` в `#[cfg(test)] mod tests`:
   - `reprocess_transcribe_enqueues_job_and_returns_job_id` — POST `/api/v1/meetings/:id/reprocess` с `{"kind":"transcribe"}`, проверить `202 Accepted`, тело `{"job_id":"..."}`, и что в `FakeJobRepo` появился job с `kind=ReprocessTranscribe`.
   - `reprocess_transcribe_404_for_unknown_meeting` — уже есть, оставить.
   - `reprocess_transcribe_422_when_audio_path_missing` — добавить, если ещё нет.

7. **QML‑тест** (новый) — `qt-app/tests/tst_generate_protocol_screen.cpp`:
   - Mount `GenerateProtocolScreen` с замоканным `ApiClient` (или с тестовым HTTP‑сервером, как сделано в `tst_pipeline_progress.cpp`).
   - Сценарий A: `hasTranscript=false` → клик «Сгенерировать» → проверить, что отправлен POST `/reprocess` с `{"kind":"transcribe"}` (НЕ POST `/transcribe`).
   - Сценарий B: после `done` транскрипции → отправлен POST `/reprocess` с `{"kind":"protocol"}` и `templateName`.
   - Сценарий C: `failed` любого job‑а → `st="failed"`, текст ошибки виден.
   - Регрессионная сторона: убедиться, что в проекте нигде не сохранился вызов `/api/v1/transcribe`.

## Verification

```bash
cargo test --manifest-path rust/Cargo.toml
cd qt-app && cmake --build build && ctest --test-dir build --output-on-failure
./run-qt.sh
```

Ручной прогон: записать 30–60 с встречу с микрофона; остановить. Убедиться, что экран генерации показывает: подсвеченный шаг «Транскрипция», бегущий процент, подстатус. Через 1–2 с после `done` шаг подсвечивается «Протокол».

## Нон‑goals

- Не трогаем кейс «пользователь ушёл с экрана» — это [active-jobs-store.md](../../../backlog/active-jobs-store.md).
- Не трогаем кейс «приложение перезапустили» — это [resume-in-flight-jobs-on-restart.md](../../../backlog/resume-in-flight-jobs-on-restart.md).
- Не добавляем серверную защиту от двойного enqueue transcribe‑job.
- Не меняем меню «Перетранскрибировать» в `MeetingDetailScreen`.
