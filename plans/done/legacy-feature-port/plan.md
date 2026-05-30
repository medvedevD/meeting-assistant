# Портирование legacy-функционала в Qt 6 + Rust sidecar

> **Расположение плана**: harness ограничивает редактирование этим файлом. После approval перенести в `<repo>/.claude/plans/legacy-feature-port/plan.md` (per CLAUDE.md guideline).

## Context

В репо есть `/legacy/` — устаревшая Python (FastAPI + HTML/JS) реализация Meeting Assistant. От неё отказались в пользу Qt 6 (QML) + Rust HTTP sidecar, но **GUI и настройки legacy ощутимо удобнее текущих**: полный экран настроек с мульти-LLM, CRUD шаблонов, drag&drop импорт аудио, "Из папки", перетранскрибировать/перегенерировать, 3-step pipeline progress с классификацией ошибок. Цель — перенести этот UX в актуальное приложение, не возвращаясь к Python.

**Главное архитектурное наблюдение**: `JsonSettingsStore` в Rust уже определён ([settings_store.rs](rust/crates/adapters/src/settings_store.rs)), но **не подключён** к sidecar — `Container::new_sidecar` ([container.rs:57-89](rust/crates/app/src/container.rs#L57-L89)) захардкоживает `TranscriberPrefs::new("ru", 1, 0)` и читает только env `ANTHROPIC_API_KEY`. Sidecar не отдаёт `/settings` через HTTP. Qt SettingsScreen хранит recording/template локально через `QtCore.Settings`, остальное явно помечает "управляется ядром". Это первая блокирующая работа.

**Решения пользователя по scope**:
- LLM: Claude (есть) + добавить OpenAI/ChatGPT, Gemini, Mistral, Ollama (локально).
- MVP включает всё одновременно: полный экран настроек, reprocess actions, импорт + drag&drop + "Из папки", pipeline progress с классификацией ошибок.
- UX настроек: macOS-style боковая навигация по категориям.
- API ключи: системный keyring (crate `keyring` 3.x), env как override.

## Зафиксированные решения (grill-me)

1. **LLM-конфиги**: хранить все 5 провайдеров одновременно в `settings.json` + поле `active`. Переключение без потери настроек.
2. **Hot-swap**: `ArcSwap<dyn LlmProvider>` + `LazyWhisperTranscriber::set_prefs`. `prompts_dir`/`recordings_dir` тоже горячие (`ArcSwap`-поля). Только `db_path` restart-required — баннер в UI (зависят long-lived Db/repos/worker).
3. **Без core-портов** для settings/secrets — `JsonSettingsStore` + `KeyringSecretStore` живут в adapters/composition root. `default_template` резолвится в API-слое перед вызовом use-case (use-case принимает готовый `template_name`).
4. **Keyring fallback**: `keyring` 3.x; при недоступности (unsigned macOS dev) — fallback на `~/.config/meeting-assistant/secrets.json` mode 0600 plaintext + баннер «ключи незашифрованы». Env-переменные приоритетнее обоих (`effective_key`).
5. **Тест ключа**: `POST /api/v1/settings/test { provider }` — дешёвый probe (Anthropic: 1-token messages; OpenAI/Mistral: `GET /models`; Gemini: list models; Ollama: `GET /api/tags`) → `ok` или `error_class`.
6. **Выбор модели в UI**: редактируемый `ComboBox` (общие модели + свободный ввод). Ollama — реальный дропдаун из `GET /api/tags`.
7. **Удаление дефолтного шаблона**: сервер очищает `default_template` (откат на встроенный) + warning в ответе, UI показывает тост.
8. **Валидация имени шаблона**: whitelist (Unicode-буквы/цифры/пробел/`-`/`_`, ≤100), имя = имя файла; отклонять, если `sanitized != input` (защита от traversal). Сохраняет совместимость с существующими кириллическими именами.
9. **Импорт аудио**: копировать исходник в `recordings_dir/<id>/` (оригинал остаётся). «Из папки» — регистрировать файлы по месту, без копирования.
10. **Дедуп при импорте**: по `audio_path` → 409 + ссылка на существующую встречу. Без хеширования содержимого.
11. **Прогресс пайплайна**: polling (`GET /jobs/:id`). Live-прогресс (stage/sub/percent) в памяти (`Arc<DashMap<JobId, Progress>>` в AppState), НЕ персистится. В БД персистится только `error_class` (одна колонка). `GET` мёржит память+БД.
12. **Qt settings-state**: QML-синглтон `SettingsStore.qml` (паттерн `MeetingStore.qml`), GET/PUT через `api`. Единственное C++ изменение — добавить `put`/`del` в `ApiClient` (сейчас только `get`/`post`).
13. **QtCore.Settings → server**: перенести `recording.source`/`echoCancel`/`defaultTemplate` на сервер (settings.json авторитетен). Экраны читают дефолты из `SettingsStore`; per-recording override локально без записи глобального дефолта.
14. **Поставка**: всё в текущей ветке `feat/qt-migration`, **коммит после каждой завершённой фазы** (не отдельные PR).

## Phases (последовательно между, параллельно внутри)

### Phase 1 — Settings backend + multi-LLM + keyring (фундамент, блокирует остальное)

**Core ports/entities**
- [rust/crates/core/src/lib.rs](rust/crates/core/src/lib.rs) — добавить варианты `CoreError`: `ApiAuth`, `ApiQuota`, `ApiTimeout`, `AudioCorrupt`, `Network`, `WorkerKilled` (база для классификации ошибок в Phase 4).
- **Без новых core-портов** (решение #3): `SettingsStore`/`SecretStore` НЕ заводим в core. Они живут в adapters; настройки/секреты используются только в composition root (Container) и API-слое.
- Рядом с `LlmProvider` trait — `ProviderKind` enum (Anthropic/OpenAI/Gemini/Mistral/Ollama) и `LlmConfig { kind, model, max_tokens, base_url, api_key }`. Сам trait `LlmProvider` НЕ меняется (подтверждено: `generate(transcript, instructions) -> String` достаточно).

**Adapters**
- [rust/crates/adapters/src/settings_store.rs](rust/crates/adapters/src/settings_store.rs) — расширить `PersistedSettings`:
  - `llm: LlmPrefs { kind, anthropic: ProviderCfg, openai, gemini, mistral, ollama }` где `ProviderCfg { model, max_tokens, base_url }`.
  - `transcriber`: добавить `model_path`, `vad: bool`, `vad_threshold: f32`.
  - **Убрать `anthropic_api_key` из JSON** — миграция: при загрузке если поле есть, перенести в keyring и затереть.
  - Реализовать `impl SettingsStore`.
- Новый `crates/adapters/src/secret_store.rs` — `KeyringSecretStore` поверх `keyring = "3"`, service `"meeting-assistant"`, keys `api_key.{anthropic,openai,gemini,mistral}`. Метод `effective_key(kind)` с env precedence. **Fallback** на mode-0600 JSON (`~/.config/meeting-assistant/secrets.json`) — критично для unsigned dev-сборок на macOS, где Keychain отказывает без подписи бандла.
- `crates/adapters/src/llm/` — новые файлы `openai.rs`, `gemini.rs`, `mistral.rs`, `ollama.rs` (структурно копия `anthropic.rs`). Общий `errors.rs` маппит HTTP 401/403→`ApiAuth`, 429→`ApiQuota`, 5xx→`Network`.
- Фабрика `crates/adapters/src/llm/factory.rs` — `build_llm(LlmConfig) -> Arc<dyn LlmProvider>`.
- `Cargo.toml`: добавить `keyring = "3"`, `arc-swap`.

**Container hot-swap**
- [rust/crates/app/src/container.rs](rust/crates/app/src/container.rs) — поле `llm` становится `Arc<ArcSwap<dyn LlmProvider>>`. `transcriber` уже `LazyWhisperTranscriber` с `set_prefs`/`set_model_path` — выставить concrete handle. `new_sidecar` читает `JsonSettingsStore::open_default()` и `KeyringSecretStore`, конфигурирует через фабрику. Добавить `reload_from_settings(&AppSettings)`.

**API endpoints**
- Новый `crates/api/src/routes/settings.rs`:
  - `GET /api/v1/settings` — **без секретов**, для каждого провайдера `{ kind, model, has_key: bool }`.
  - `PUT /api/v1/settings` — full DTO; save → `reload`; вернуть effective state.
  - `PUT /api/v1/settings/secret` — `{ provider, value | null }` → `SecretStore`.
- Регистрация в [router.rs](rust/crates/api/src/router.rs); `AppState` получает `settings_store`, `secret_store`, `reload` closure.

**Риски**: гонка `PUT /settings` ↔ in-flight job (mitigation: worker держит свой `Arc<dyn Transcriber>`, LLM используется синхронно — перезагрузка между запросами безопасна). Keyring без подписи на macOS → fallback обязателен.

---

### Phase 2 — Templates CRUD

**Core**
- [crates/core/src/ports/template_loader.rs](rust/crates/core/src/ports/template_loader.rs) — расширить trait: `save(name, body)`, `delete(name)`, `rename(old, new)`.
- Use-cases в `crates/core/src/usecases/templates.rs` — место для валидации (запрет path traversal, безопасный regex имени, обрезка длины).

**Adapters**
- [crates/adapters/src/templates.rs](rust/crates/adapters/src/templates.rs) — `tokio::fs` write/delete, atomic rename через tempfile.

**API**
- Новый `crates/api/src/routes/templates.rs`:
  - `GET /api/v1/templates` (list + bodies для превью)
  - `GET/PUT /api/v1/templates/:name`
  - `DELETE /api/v1/templates/:name`
  - `POST /api/v1/templates/:name/rename` `{ new_name }`

**Edge case**: удаление шаблона, на который ссылается `default_template` в settings → сервер очищает ссылку и возвращает warning.

---

### Phase 3 — Reprocess + Import + "Из папки"

**Core**
- [crates/core/src/entities/job.rs](rust/crates/core/src/entities/job.rs) — расширить `JobKind`: добавить `ReprocessTranscribe`, `RegenerateProtocol`.
- `MeetingRepo`: добавить `delete_audio_only(id)` (NULL audio_path, сохранить остальное), `clear_transcript(id)`, `clear_protocol(id)`.
- Новые use-cases в `crates/core/src/usecases/`:
  - `import_audio.rs` — копирует/перемещает файл в `recordings_dir/<id>/`, создаёт Meeting, опц. enqueue Transcribe.
  - `scan_recordings_dir.rs` — "Из папки": возвращает .wav/.mp3/.m4a без `Meeting` в БД (лимит depth, early-stop).
  - `reprocess_transcribe.rs` — clear transcript+protocol, enqueue новый job.
  - `regenerate_protocol.rs` — берёт текущий transcript, LLM с новым template, перезаписывает.
  - `delete_meeting.rs` (audio-only / full).

**Adapters**
- `crates/adapters/src/db/meeting_repo.rs` — реализовать новые методы.
- `crates/adapters/src/worker.rs` — диспатчинг по `JobKind` (сейчас только `Transcribe`).

**API** ([crates/api/src/routes/meetings.rs](rust/crates/api/src/routes/meetings.rs))
- `POST /api/v1/meetings/import` — принимает `{ path }` (локальный путь от Qt FileDialog / drag&drop URI). **Не multipart** — это локальное приложение, лишний IO не нужен. Дедупликация по `find_by_audio_path` → 409 с id существующего.
- `GET /api/v1/meetings/scan?dir=...` — untracked файлы.
- `POST /api/v1/meetings/:id/reprocess` `{ kind: "transcribe"|"protocol", template_name?: ... }` → job_id.
- `DELETE /api/v1/meetings/:id?mode=audio|full`.

---

### Phase 4 — Pipeline progress + классификация ошибок

**MVP — polling, не SSE**. SSE можно как Phase 4b позже.

**Core**
- `entities/job.rs`: `progress: JobProgress { stage: PipelineStage, sub: String, percent: u8 }`, `error_class: Option<ErrorClass>`.
  - `PipelineStage`: `Queued | LoadingModel | DecodingAudio | Transcribing | WritingTranscript | GeneratingProtocol | Done`.
  - `ErrorClass`: `AudioCorrupt | ApiQuota | ApiAuth | NetworkTimeout | WorkerKilled | ModelMissing | Unknown`.
- `JobRepo::update_progress(id, stage, sub, percent)`.

**Live-прогресс — в памяти** (решение #11): `Arc<DashMap<JobId, JobProgress>>` в `AppState`, обновляется worker'ом на каждый whisper-callback, НЕ персистится. Только `error_class` идёт в БД.

**Adapters**
- `worker.rs` — писать stage/percent в in-memory map; маппить `CoreError → ErrorClass` в финальной ошибке и персистить `error_class`.
- `db/job_repo.rs` — миграция `ALTER TABLE jobs ADD COLUMN error_class TEXT` (одна колонка; guards для NULL на старых строках).
- `whisper.rs` — `whisper-rs::set_progress_callback` для percent внутри `Transcribing` → в in-memory map.

**API** — `GET /api/v1/jobs/:id` мёржит in-memory live-прогресс (если job активен) + персистентное терминальное состояние из БД (`error_class`). (SSE в Phase 4b опционально.)

---

### Phase 5 — Qt UI

**Settings (macOS-style sidebar)**
- Переписать [qt-app/qml/screens/SettingsScreen.qml](qt-app/qml/screens/SettingsScreen.qml): `RowLayout` = `ListView` категорий слева + `StackLayout` справа.
- Новая директория `qt-app/qml/screens/settings/`:
  - `WhisperPanel.qml` — model (FileDialog для `.bin`), language ComboBox (+ "auto"), beam_size, n_threads, VAD switch + threshold.
  - `LlmPanel.qml` — provider ComboBox; динамические поля model/max_tokens/base_url/api_key; кнопка "Проверить ключ" (POST `/api/v1/settings/test`).
  - `TemplatesPanel.qml` — список + edit area + кнопки New/Delete/Rename через REST.
  - `StoragePanel.qml` — paths (recordings_dir, db, prompts).
  - `RecordingPanel.qml` — текущая логика source/echoCancel.
- Новый `qt-app/qml/SettingsStore.qml` singleton — кеш snapshot из `GET /settings`, метод `apply()` → `PUT /settings`. Удалить `QtCore.Settings`-логику для серверных ключей.

**Import & "Из папки"**
- Расширить [NewRecordingScreen.qml](qt-app/qml/screens/NewRecordingScreen.qml) (или новый `ImportScreen.qml`):
  - `DropArea` поверх (Qt `Drag.formats: ["text/uri-list"]`).
  - Кнопка "Импорт файла" → FileDialog.
  - Кнопка "Из папки" → `GET /meetings/scan` → ListView с чекбоксами и "Импортировать выбранные".
- В [MeetingStore.qml](qt-app/qml/MeetingStore.qml) — методы `importFile(path)`, `scanFolder(dir)`.

**Reprocess в детальном экране**
- [MeetingDetailScreen.qml](qt-app/qml/screens/MeetingDetailScreen.qml) — `…` меню: "Перетранскрибировать", "Перегенерировать протокол", "Удалить аудио (оставить транскрипт)", "Удалить встречу". Каждое → REST + refresh.

**Pipeline progress**
- Новый `qt-app/qml/components/PipelineProgress.qml` — 3 шага с подсветкой текущего stage, sub-status, percent. Используется в `MeetingDetailScreen` и `NewRecordingScreen` пока job активен.
- ApiClient (вероятно `qt-app/src/ApiClient.cpp` — нужно глянуть на старте Phase 5) — добавить `pollJob(id, onUpdate)` (каждую сек).
- Маппинг `error_class` → локализованный текст: новый `qt-app/qml/i18n/errors.js` (Qt i18n уже есть).

---

### Phase 6 — Polish

- `generate_protocol` use-case → читать `default_template` из `settings_store.snapshot()`, если нет в запросе.
- Юнит-тесты: `KeyringSecretStore` (mockable backend), каждый LLM-провайдер (через `wiremock`), templates CRUD (tempdir), `MeetingRepo::delete_audio_only`.
- Контрактный тест `app/tests/sidecar_contract.rs` — все новые роуты в перечне auth-gated.
- Дополнить CLAUDE.md разделами "Settings persistence and hot-swap" + "Adding a new LLM provider".

---

## Critical files to modify

| Файл | Phase | Что |
|---|---|---|
| [rust/crates/adapters/src/settings_store.rs](rust/crates/adapters/src/settings_store.rs) | 1 | Расширить `PersistedSettings`, реализовать `SettingsStore` trait, миграция API ключа в keyring |
| [rust/crates/app/src/container.rs](rust/crates/app/src/container.rs) | 1 | Подключить `JsonSettingsStore` + `KeyringSecretStore`, `ArcSwap<dyn LlmProvider>`, `reload_from_settings` |
| [rust/crates/api/src/router.rs](rust/crates/api/src/router.rs) | 1, 2, 3 | Регистрация `/settings`, `/templates`, `/meetings/{import,scan,reprocess}` |
| `rust/crates/adapters/src/secret_store.rs` (new) | 1 | KeyringSecretStore + JSON fallback |
| `rust/crates/adapters/src/llm/{factory,openai,gemini,mistral,ollama}.rs` (new) | 1 | Multi-LLM |
| [rust/crates/core/src/entities/job.rs](rust/crates/core/src/entities/job.rs) | 3, 4 | Новые JobKind, progress, error_class |
| [rust/crates/adapters/src/templates.rs](rust/crates/adapters/src/templates.rs) | 2 | CRUD |
| [rust/crates/adapters/src/worker.rs](rust/crates/adapters/src/worker.rs) | 3, 4 | Диспатч JobKind, прогресс stages |
| [qt-app/qml/screens/SettingsScreen.qml](qt-app/qml/screens/SettingsScreen.qml) | 5 | Полная переработка под sidebar+stack |
| [qt-app/qml/screens/MeetingDetailScreen.qml](qt-app/qml/screens/MeetingDetailScreen.qml) | 5 | Reprocess actions |
| [qt-app/qml/screens/NewRecordingScreen.qml](qt-app/qml/screens/NewRecordingScreen.qml) | 5 | Drag&drop + Import + "Из папки" |
| [qt-app/qml/MeetingStore.qml](qt-app/qml/MeetingStore.qml) | 5 | importFile/scanFolder |
| `qt-app/qml/components/PipelineProgress.qml` (new) | 5 | 3-step progress UI |
| `qt-app/qml/SettingsStore.qml` (new) | 5 | Серверные настройки в Qt |

---

## Reuse — что уже есть в коде

- `LazyWhisperTranscriber::set_prefs` / `set_model_path` ([rust/crates/adapters/src/whisper.rs](rust/crates/adapters/src/whisper.rs)) — готовый hot-swap для transcriber, не надо изобретать.
- `JsonSettingsStore::open_default` + миграция из старого Tauri-store ([settings_store.rs:74-143](rust/crates/adapters/src/settings_store.rs#L74-L143)) — переиспользовать паттерн миграции для переноса `anthropic_api_key` → keyring.
- `FileTemplateLoader` ([rust/crates/adapters/src/templates.rs](rust/crates/adapters/src/templates.rs)) — основа для CRUD, расширить.
- `Anthropic` adapter ([rust/crates/adapters/src/llm/anthropic.rs](rust/crates/adapters/src/llm/anthropic.rs)) — шаблон для остальных провайдеров (один `reqwest::Client`, одна функция request/parse).
- AppShell `showSettings` навигация ([qt-app/qml/AppShell.qml:65-69](qt-app/qml/AppShell.qml#L65-L69)) — уже есть, расширять не надо.
- legacy `/legacy/meeting_assistant/interfaces/web/static/` — справочник для UX-решений (структура секций, валидация ключей, статус-индикаторы).

---

## Verification (end-to-end проверки после каждой фазы)

**После Phase 1**:
- `cargo test --manifest-path rust/Cargo.toml` — все юнит-тесты зелёные.
- Запустить `./run-qt.sh --debug`, открыть Settings: GET `/api/v1/settings` отдаёт корректный JSON; PUT сохраняет и сразу применяется (проверить: поменять язык на en, перетранскрибировать — Whisper использует en).
- Установить API ключ Anthropic через PUT `/settings/secret`, проверить `security find-generic-password -s meeting-assistant -a api_key.anthropic` на macOS.
- На системе без keyring (или fake-env) — fallback в `~/.config/meeting-assistant/secrets.json` с mode 0600.

**После Phase 2**:
- POST/PUT/DELETE шаблона через UI; проверить файл на диске в `/prompts/`.
- Создать шаблон с именем `../../evil` → отказ.

**После Phase 3**:
- Drag&drop .wav в Qt → встреча создаётся, транскрипция стартует автоматически.
- "Из папки" → положить .wav в recordings_dir вручную, сканировать → встреча видна в кандидатах.
- "Удалить аудио" → файл удалён, транскрипт+протокол остались, лист встреч показывает.
- "Перегенерировать протокол" с новым шаблоном → новый файл, старый перезаписан.

**После Phase 4**:
- В новой встрече видны 3 шага pipeline + percent + sub-status.
- Сломать ANTHROPIC_API_KEY → ошибка показывает класс `ApiAuth` с локализованным сообщением и кнопкой "Открыть настройки".

**После Phase 5**:
- Полный сценарий: открыть свежий профиль → пустые настройки → ввести ключи 4 провайдеров → импортнуть аудио drag&drop → перетранскрибировать с en → перегенерировать с новым шаблоном через Gemini.

**После Phase 6**:
- `cargo test --manifest-path rust/Cargo.toml` — все юнит/контрактные.
- `cd legacy && pytest` (legacy не трогаем, регрессий не должно быть; на самом деле опционально, legacy скоро удалят).

---

## Открытые вопросы (стоит проверить на старте имплементации)

1. Где живёт Qt `ApiClient` (`qt-app/src/ApiClient.{h,cpp}` — не успел прочесть) — насколько простой add-method API для нового `/settings`, `/templates`, `/meetings/import`.
2. Какой именно crate `keyring` использовать (3.x vs 2.x) — проверить совместимость с macOS Sequoia + статус подписи бандла.
3. SSE сейчас или polling — рекомендация в плане: **polling в MVP**, SSE как Phase 4b.
4. Сохранять `recording.source` / `echo_cancel` / `default_template` в server-side settings или оставить QtCore.Settings — рекомендация: **server-side** (единый источник).
