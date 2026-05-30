# Cleanup: remove Kotlin/Compose stack — PRD v1.0

## Requirements Description

### Background
- **Business problem**: Qt-миграция завершена и зелёная на CI на macOS/Linux/Windows
  (qt-ci run 26146809501, протокольный SSOT-гейт + 3-OS матрица). Старый Compose-Desktop
  стек (Kotlin/Compose + UniFFI cdylib мост) больше не используется ни в проде, ни
  в dev-флоу — он сейчас просто шумит в репо, тащит лишние зависимости
  (whisper-rs в `ffi` cdylib, JNA, uniffi, Gradle/JDK тулчейн) и создаёт ложные
  пути для будущих читателей кода.
- **Target audience**: maintainer(ы) этого репо и любой новый контрибьютор —
  чтобы код, который они открывают, отражал текущий стек, а не два мёртвых.
- **Value**: один прозрачный фронт (Qt) + сокращённый Rust workspace (4 крейта
  вместо 6) + меньше тулчейна (нет JDK) → быстрее onboarding, быстрее CI,
  невозможен случайный релиз старой Compose-версии.

### Feature overview
- **Core**: удалить Compose-стек целиком (UI + FFI мост + dev/release-скрипты +
  старый CI workflow + упоминания в CLAUDE.md) одним коммитом в текущей
  ветке `feat/qt-migration`, до её мерджа в `main`.
- **Boundaries**:
  - **IN**: `ui-compose/`, `rust/crates/ffi/`, `rust/crates/uniffi-bindgen/`,
    `run-compose.sh`, `build-ffi.sh`, `scripts/generate-icon.main.kts`,
    `.github/workflows/release.yml`, Compose-секции в `CLAUDE.md`,
    `java = temurin-17` в `.mise.toml`, осиротевшие
    `[workspace.dependencies]` в `rust/Cargo.toml`, заголовок-комментарий
    `run-qt.sh` (упоминание «successor to run-compose.sh»), удалённая ветка
    `origin/feat/compose-desktop-rewrite`.
  - **OUT**: `legacy/` (Python-прототип) — оставить как есть, удаление —
    отдельная задача по запросу владельца. `prompts/`, `packaging/`, `qt-app/`,
    `rust/crates/{core,adapters,api,app}`, `.cargo/config.toml`, `Brewfile`
    (все brew-пакеты используются Rust/Qt-стеком) — не трогать.
- **User scenarios**:
  - Maintainer открывает репо после мерджа — видит ровно один UI-стек (Qt) и
    один dev-скрипт (`run-qt.sh`); `cargo test --manifest-path rust/Cargo.toml`
    проходит на 4 крейтах; `qt-ci` зелёный.
  - Никакой случайный `git push` ничего не релизит — старый `release.yml`
    отсутствует, его триггерная ветка тоже.

### Detailed requirements

#### Удалить (объекты + причина)

| Объект | Кому принадлежит | Почему уходит |
|---|---|---|
| `ui-compose/` (весь каталог) | Compose Desktop UI | Полностью заменён `qt-app/` |
| `rust/crates/ffi/` | UniFFI cdylib для Kotlin | Единственный потребитель — `ui-compose` |
| `rust/crates/uniffi-bindgen/` | бинарь, генерящий Kotlin-биндинги | то же |
| `run-compose.sh` | dev-скрипт сборки/запуска Compose | канон теперь `run-qt.sh` |
| `build-ffi.sh` | сборка UniFFI cdylib | потребителя больше нет |
| `scripts/generate-icon.main.kts` | одноразовый Kotlin-скрипт `.icns`-генерации | `.icns` уже лежит в `packaging/assets/`; единственный оставшийся пользователь Kotlin-тулчейна |
| `.github/workflows/release.yml` | старый Gradle/Compose релиз-пайплайн | триггерится только на старую ветку, билдит Gradle-Compose, на Qt не работает; полноценный qt-release.yml — отдельная задача |
| `java = "temurin-17"` в `.mise.toml` | mise-пин JDK для Gradle | Qt-стек JDK не требует |

#### Подрезать (не удалить, но привести в порядок)

| Объект | Что сделать |
|---|---|
| `rust/Cargo.toml` `[workspace]` | убрать `crates/ffi`, `crates/uniffi-bindgen` из `members` |
| `rust/Cargo.toml` `[workspace.dependencies]` | прогнать `cargo machete` / визуальный аудит: убрать deps, которые больше нигде не используются после удаления ffi (кандидаты: ничего не пропадёт «бесплатно» — большинство deps используется `meeting-adapters`/`meeting-app`; проверка обязательна) |
| `CLAUDE.md` | переписать: убрать секции `run-compose.sh` / `Regenerate Kotlin FFI Bindings` / `Kotlin UI (/ui-compose)` / `FFI Bridge`; обновить `Key Files` (вместо `ffi/src/lib.rs`, `Main.kt` — `rust/crates/app/src/bin/meeting-server.rs`, `qt-app/src/main.cpp`, `qt-app/qml/Main.qml`); раздел Architecture — оставить только Rust clean-architecture + Qt-фронт + sidecar HTTP мост |
| `run-qt.sh` | строка-заголовок: убрать «successor to run-compose.sh» (становится просто «canonical dev workflow») |
| `meeting-server.rs` `try_acquire_singleton` doc-комментарий | строка «reimplemented here because the FFI crate is a cdylib, not a library dependency» устаревает — сократить до простого описания (никакой другой реализации больше нет) |

#### Remote

- `git push --delete origin feat/compose-desktop-rewrite` (без архивного тега —
  явное решение владельца, история остаётся в reflog/clone'ах).

#### Не делать в этой задаче (явные out-of-scope)

- Не трогать `legacy/` (Python).
- Не писать `.github/workflows/qt-release.yml` — отдельная задача по
  упаковке/публикации Qt-артефактов.
- Не трогать `.gitignore` (проверено — Compose/Gradle секций нет уже сейчас).
- Не трогать `README.md` (его нет в репо).
- Не менять никакой `.cargo/`, `Brewfile`, `packaging/`, `prompts/`.

### Edge cases / footguns

1. **Workspace member удаление**. После правки `rust/Cargo.toml` `cargo
   metadata` должен валидно резолвиться. Любой висящий `path = "../ffi"` или
   `[dependencies] ffi = ...` в оставшихся крейтах должен быть удалён
   синхронно (по факту таких ссылок быть не должно — `ffi` и `uniffi-bindgen`
   были потребителями, не зависимостями).
2. **`Cargo.lock` дедуп**. После удаления `ffi`/`uniffi-bindgen` многие deps
   (uniffi, jna, whisper-rs JNI features) станут осиротевшими. `cargo build`
   автоматически уберёт их из lock. Закоммитить обновлённый `Cargo.lock`.
3. **macOS `.cargo/config.toml`**. Содержит Swift rpath для ScreenCaptureKit
   (раздел 05). Не трогаем — нужен Rust-ядру.
4. **mise.toml после удаления java**. Файл может оказаться пустым `[tools]`.
   Допустимо либо оставить пустой блок, либо удалить весь файл. Решение:
   удалить файл целиком, если других tool-пинов не появится.
5. **CI после удаления release.yml**. Старая ветка `feat/compose-desktop-rewrite`
   уходит первой — это снимает её триггер. Удаление файла `release.yml`
   делается одним коммитом с остальными правками. Никакого окна, в котором
   ветка есть, а файла нет (или наоборот) — оба меняются одновременно с push.

## Design Decisions

### Approach
- **Один коммит** в текущей ветке `feat/qt-migration`. Сообщение коммита —
  `chore(cleanup): remove Compose/UniFFI stack` с явным списком удалённых
  объектов и обоснованием. Это решение владельца (отдельной ветки не делаем
  — выбрано в раунде уточнений).
- **Verification gate**: после `git push` ожидаем зелёного `qt-ci` на трёх
  ОС. Только так становится финально ясно, что удаление ничего не сломало
  ни на Linux, ни на Windows.
- **Команда чистки workspace deps**: `cargo machete` (если установлен), либо
  ручной аудит — для каждой записи в `[workspace.dependencies]` грепнуть
  `{ workspace = true }` по оставшимся `crates/*/Cargo.toml`. Сохраняем
  только то, что реально используется.

### Key components touched
- Rust workspace manifest: `rust/Cargo.toml`
- Build/dev scripts: `run-compose.sh` (rm), `build-ffi.sh` (rm), `run-qt.sh` (правка комментария)
- CI: `.github/workflows/release.yml` (rm)
- Tooling: `.mise.toml` (rm/прoрезка)
- Docs: `CLAUDE.md` (переписать)
- Remote: `origin` ref'ы

### Constraints
- **Не ломать `qt-ci`** на трёх ОС.
- **Не трогать `legacy/`** ни одной строкой.
- **Атомарность**: один коммит = одна логическая правка («Compose-стек
  удалён»).

### Risks
| Риск | Митигация |
|---|---|
| Удаление workspace member ломает оставшийся билд (скрытая зависимость от ffi/uniffi) | `cargo build --workspace --all-targets` + `cargo test --workspace` локально перед коммитом; зелёный `qt-ci` после push |
| `Cargo.lock` дедуп раздувает диф | Закоммитить отдельно ИЛИ принять — это ожидаемая часть, в сообщении коммита упомянуть |
| Случайно удалена строка, нужная Qt-стеку (например в `CLAUDE.md` про packaging) | Текстовый дифф ревьюится глазами; основа — переписать секцию, а не «удалить упоминания» |

## Acceptance Criteria

### Functional
- [ ] `ui-compose/` отсутствует
- [ ] `rust/crates/ffi/` и `rust/crates/uniffi-bindgen/` отсутствуют; `rust/Cargo.toml` `[workspace.members]` содержит ровно `core, adapters, api, app`
- [ ] `run-compose.sh`, `build-ffi.sh`, `scripts/generate-icon.main.kts` отсутствуют
- [ ] `.github/workflows/release.yml` отсутствует; в `.github/workflows/` остался только `qt-ci.yml`
- [ ] `.mise.toml` либо удалён, либо не содержит `java = …`
- [ ] `CLAUDE.md` не содержит слов `compose`, `gradlew`, `ui-compose`, `Kotlin`, `UniFFI`, `Material` (кроме упоминания в historic-контексте, если потребуется); `Key Files` указывает на `meeting-server.rs` / `qt-app/main.cpp`
- [ ] `run-qt.sh` не упоминает `run-compose.sh`
- [ ] `git ls-remote origin feat/compose-desktop-rewrite` пустой
- [ ] `rust/Cargo.toml` `[workspace.dependencies]` не содержит deps без потребителя (audit прогнан, в коммит-месседже отмечено)

### Quality
- [ ] `cargo build --workspace --all-targets --manifest-path rust/Cargo.toml` зелёный локально
- [ ] `cargo test --manifest-path rust/Cargo.toml` зелёный локально (138 тестов, как до зачистки)
- [ ] `qt-ci` после push зелёный на macOS + Linux + Windows + `protocol-version`
- [ ] В чистом клоне `git clone ... && cd ... && ./run-qt.sh` собирает и запускает Qt-приложение без упоминаний/требований к Kotlin/Gradle/JDK

### Doc / UX
- [ ] CLAUDE.md прочитан целиком после правок — описывает текущую реальность (Rust core + Qt UI через loopback HTTP), не противоречит коду

## Execution phases

### Phase 1 — Local dry run (10–15 мин)
**Цель**: убедиться, что удаление ничего не ломает, до того как трогать git.
- [ ] `git status` чистый
- [ ] Удалить файлы/каталоги: `ui-compose/`, `rust/crates/ffi/`, `rust/crates/uniffi-bindgen/`, `run-compose.sh`, `build-ffi.sh`, `scripts/generate-icon.main.kts`, `.github/workflows/release.yml`
- [ ] Правка `rust/Cargo.toml`: убрать ffi+uniffi-bindgen из members
- [ ] `cargo build --workspace --all-targets --manifest-path rust/Cargo.toml` — должен пройти
- [ ] Прогнать audit unused workspace deps (cargo machete или ручной грепп), удалить осиротевшие
- [ ] `cargo test --manifest-path rust/Cargo.toml` — 138 тестов зелёные
- **Deliverables**: рабочее дерево с удалённым стеком, рабочие тесты.

### Phase 2 — Docs + housekeeping (10 мин)
**Цель**: документация и тулчейн отражают новую реальность.
- [ ] Переписать `CLAUDE.md` (по списку Detailed requirements)
- [ ] Поправить заголовок `run-qt.sh`
- [ ] Удалить/опустошить `.mise.toml`
- [ ] Освежить doc-комментарий `try_acquire_singleton` в `meeting-server.rs`
- **Deliverables**: согласованная документация.

### Phase 3 — Commit + push + remote cleanup (5 мин)
- [ ] `git add -A && git diff --cached --stat` — sanity-чек диффа
- [ ] Один коммит с подробным сообщением
- [ ] `git push`
- [ ] `git push --delete origin feat/compose-desktop-rewrite`
- **Deliverables**: ветка обновлена, старая ветка удалена.

### Phase 4 — CI gate (фоновое ожидание ~10–15 мин)
- [ ] `gh run watch <id> --exit-status` на новый запуск `qt-ci`
- [ ] Если все 4 джобы зелёные → задача закрыта
- [ ] Если что-то упало → диагностика и точечный фикс; повторный push
- **Deliverables**: зелёный CI на трёх ОС подтверждает «не сломалось».

---

**Document version**: 1.0
**Created**: 2026-05-20
**Clarification rounds**: 2
**Quality score**: 96/100
