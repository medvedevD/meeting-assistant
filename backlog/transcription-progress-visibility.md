# Transcription Progress Visibility

**Status:** Backlog

## Context

После остановки длинной записи (например, часовой встречи) пользователь видит
индикатор обработки без процента готовности. Причина — `GenerateProtocolScreen`
для шага транскрипции ходит в синхронный `POST /api/v1/transcribe`
([transcribe.rs](../rust/crates/api/src/routes/transcribe.rs)), который блокирует
запрос до конца расшифровки и ничего не сообщает о прогрессе. В UI крутится
indeterminate‑полоска без процента и без подстадий
([GenerateProtocolScreen.qml:383-407](../qt-app/qml/screens/GenerateProtocolScreen.qml#L383-L407)).

При этом транскрибирующий воркер уже умеет писать прогресс в общую
`progress` DashMap (стадии `loading_model / decoding_audio / transcribing /
writing_transcript`), а `PipelineProgress` ([PipelineProgress.qml](../qt-app/qml/components/PipelineProgress.qml))
уже умеет рисовать процент и стадии — этим механизмом пользуется только шаг
«Протокол».

Для шага «транскрибировать» уже есть асинхронный маршрут
`POST /api/v1/meetings/:id/reprocess {kind:"transcribe"}`
([meetings.rs:217](../rust/crates/api/src/routes/meetings.rs#L210)), который
ставит job в очередь и возвращает `job_id`.

## Goal

Показывать процент и подстадию транскрипции точно так же, как сейчас для шага
«Протокол».

## Approach

- В `GenerateProtocolScreen` заменить ветку «нет транскрипта» на enqueue job‑а
  `kind:"transcribe"` через `/reprocess` вместо синхронного `/transcribe`.
- Полить тот же job через `PipelineProgress` — стадии транскрипции уже описаны
  в маппинге компонента.
- По завершении этого job‑а — enqueue’ить protocol‑job и продолжить штатным
  путём.
- Удалить (или оставить только для CLI) использование синхронного
  `/api/v1/transcribe` из QML.

## Expected Outcome

- Во время транскрипции в UI видны процент и текущий шаг (модель грузится /
  декодирование / Whisper распознаёт / сохранение).
- Поведение «Перетранскрибировать» из меню детали встречи и автозапуск после
  остановки записи используют один и тот же job‑механизм.
