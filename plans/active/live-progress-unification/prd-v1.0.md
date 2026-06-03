# Live Progress Unification

## Problem

`LiveProgress = Arc<DashMap<String, JobProgress>>` объявлен дважды —
параллельными type-alias'ами в
[`meeting-api::router`](../../../rust/crates/api/src/router.rs#L22) (reader,
используется `GET /jobs/:id` и `GET /active-jobs`) и в
[`meeting-adapters::worker`](../../../rust/crates/adapters/src/worker.rs#L22)
(writer). Оба раскрываются в один и тот же конкретный тип, и сегодня
композиционный слой
([`app/src/container.rs:50`](../../../rust/crates/app/src/container.rs#L50))
случайно использует адаптерный alias для поля, читаемого API-слоем — это
работает только потому, что type aliases в Rust — структурное равенство,
а не номинальное. Любая миграция одной стороны (обёртка `LiveProgress` в
`struct` для будущей задачи) скомпилируется в своей crate и тихо разойдётся
с другой. Будущие задачи ([[worker-concurrency-pool]] уже в active,
[[job-progress-sse]], [[job-cancellation]]) ожидают единый стабильный тип.

## Goal

Одна точка объявления `LiveProgress` в `meeting-core`. Reader (api) и writer
(adapters) ссылаются на один и тот же `meeting_core::LiveProgress` —
структурно **и** номинально. Поведения не меняем.

## Scope

**В этой задаче:**

- Перенести `pub type LiveProgress = Arc<DashMap<String, JobProgress>>` в
  `meeting-core` (см. Decisions для точного места).
- Добавить `dashmap` в зависимости `meeting-core` (workspace-декларация
  `dashmap = "6"` уже есть).
- Удалить локальные alias-объявления в `meeting-api::router` и
  `meeting-adapters::worker`; обновить `use`/`pub use` так, чтобы внешние
  call-site'ы продолжали писать `meeting_adapters::LiveProgress` и
  `meeting_api::LiveProgress` без правок (re-export).
- Compile-time проверка, что оба пути ведут к одному ядерному типу
  (ручной `fn _same(_: meeting_api::LiveProgress) -> meeting_adapters::LiveProgress`
  в test-модуле `meeting-app`).

**Вне этой задачи:**

- Замена type-alias'а на newtype/struct (например,
  `pub struct LiveProgress(Arc<DashMap<...>>)`) — обсуждается отдельно как
  часть SSE/cancellation API (нужна, когда захотим повесить методы или
  `Drop`); сейчас alias достаточно.
- Перенос `JobProgress` куда-либо ещё — уже в core, не трогаем.
- Любые изменения логики чтения/записи прогресса, payload `GET /jobs/:id`
  и `GET /active-jobs`.

## Decisions

1. **Местоположение в core: новый модуль `core/src/live.rs`, не
   `entities/job.rs`.** Сущности (entities) держим чистыми доменными
   типами — `Arc<DashMap<...>>` это runtime-инфраструктура для одной
   in-memory таблицы, а не доменное понятие. `mod live; pub use
   live::LiveProgress;` в `core/src/lib.rs`.
   - *Альтернатива* (положить в `entities/job.rs` рядом с `JobProgress`,
     как буквально предлагает backlog) отброшена: тянет `dashmap` в
     модуль чистых данных и затирает разделение Entities vs runtime-state.
   - *Статус:* твёрдо.

2. **`dashmap` — обычная (не feature-gated) зависимость `meeting-core`.**
   `workspace.dashmap = "6"` уже декларирован в
   [`rust/Cargo.toml`](../../../rust/Cargo.toml#L19); добавить
   `dashmap = { workspace = true }` в `core/Cargo.toml`. Без feature-флага —
   тип публичный, и `fakes`-фича его не покрывает.
   - *Статус:* твёрдо.

3. **API сохраняем re-export'ами.** В `meeting-api::router` и
   `meeting-adapters::worker` место бывшего `pub type` занимает
   `pub use meeting_core::LiveProgress;`. Внешние call-site'ы
   (`container.rs`, `routes/jobs.rs`, тесты в `router.rs::tests`) не
   правятся.
   - *Почему:* фокус задачи — устранить дрейф, а не миграция импортов.
   - *Статус:* твёрдо.

4. **Compile-time проверка — ручная функция в `meeting-app`-тесте.**
   `meeting-app` — единственная crate, которая зависит и от
   `meeting-api`, и от `meeting-adapters`. В `meeting-core` обе верхние
   crate'ы не видны, в `meeting-api`/`meeting-adapters` видна только
   `meeting-core`. Тест в `meeting-app/tests/live_progress_unified.rs`.
   - *Почему функция, а не `static_assertions::assert_type_eq_all!`:*
     `static_assertions` не в workspace; зависимость ради одного теста
     избыточна. `fn round_trip(p: meeting_api::LiveProgress) ->
     meeting_adapters::LiveProgress { p }` компилируется ↔ типы
     номинально равны.
   - *Статус:* твёрдо.

5. **Type alias, не newtype.** `pub type LiveProgress = Arc<DashMap<...>>;`
   а не `pub struct LiveProgress(Arc<DashMap<...>>)`. Текущие потребители
   (`Worker::run_*` и `routes/jobs.rs`) напрямую вызывают `DashMap` API
   (`insert/get/remove/iter`); newtype потребовал бы либо обёрток-методов,
   либо `pub`-поля, расширяя blast radius за пределы цели. Newtype
   откладывается до момента, когда понадобится lifecycle-логика
   (cleanup на терминальных статусах, snapshot для SSE) — естественная
   точка вместе с [[job-progress-sse]] / [[job-cancellation]].
   - *Статус:* твёрдо для этой задачи; пересмотр в PRD следующих задач.

## Deliverables

### Backend

**1. `rust/crates/core/Cargo.toml`** — добавить:

```toml
dashmap = { workspace = true }
```

**2. `rust/crates/core/src/live.rs`** (новый файл):

```rust
use crate::entities::JobProgress;
use dashmap::DashMap;
use std::sync::Arc;

/// Live, in-memory job-progress table keyed by job id. Shared between the
/// worker (writer) and the `GET /jobs/:id` / `GET /active-jobs` handlers
/// (readers). Never persisted (decision #11 — see
/// `plans/done/active-jobs-store`).
pub type LiveProgress = Arc<DashMap<String, JobProgress>>;
```

**3. `rust/crates/core/src/lib.rs`** — добавить:

```rust
mod live;
pub use live::LiveProgress;
```

**4. `rust/crates/api/src/router.rs`** — заменить локальное объявление
(line 22):

```diff
- pub type LiveProgress = Arc<DashMap<String, JobProgress>>;
+ pub use meeting_core::LiveProgress;
```

Снять `use dashmap::DashMap;` (line 10) — становится unused (тест на line 338
использует fully-qualified `dashmap::DashMap::new()`). `use
meeting_core::entities::JobProgress` остаётся (используется в tests-модуле).
`Arc` остаётся.

**5. `rust/crates/adapters/src/worker.rs`** — заменить локальное объявление
(line 22):

```diff
- pub type LiveProgress = Arc<DashMap<String, JobProgress>>;
+ pub use meeting_core::LiveProgress;
```

`use dashmap::DashMap;` (line 1) **оставить** — используется в tests-модуле
на lines 389/523/708 (`Arc::new(DashMap::new())`).

**6. `rust/crates/adapters/src/lib.rs`** — `pub use worker::{LiveProgress, ...}`
**оставить как есть**. Re-export через worker re-exportит то, что worker
re-exportит из core — внешние пути `meeting_adapters::LiveProgress` и
`meeting_adapters::worker::LiveProgress` указывают на тот же
`meeting_core::LiveProgress`.

**7. `rust/crates/api/src/lib.rs`** — `pub use router::{LiveProgress, ...}`
аналогично оставить.

### Тесты

**1. `rust/crates/app/tests/live_progress_unified.rs`** (новый файл):

```rust
//! Compiles iff `meeting_api::LiveProgress` and `meeting_adapters::LiveProgress`
//! resolve to the same nominal type (the one defined in `meeting_core`).
//! Guards against accidental re-introduction of parallel type aliases.

#[allow(dead_code)]
fn api_to_adapters(p: meeting_api::LiveProgress) -> meeting_adapters::LiveProgress {
    p
}

#[allow(dead_code)]
fn core_to_api(p: meeting_core::LiveProgress) -> meeting_api::LiveProgress {
    p
}

#[allow(dead_code)]
fn core_to_adapters(p: meeting_core::LiveProgress) -> meeting_adapters::LiveProgress {
    p
}
```

(Без `#[test]`-функций — достаточно того, что файл компилируется в режиме
тестовой сборки.)

**2. Существующие тесты не правим.** Конкретно:
- `api/src/routes/jobs.rs:245,297` (`let progress: crate::router::LiveProgress = std::sync::Arc::new(dashmap::DashMap::new());`) — `crate::router::LiveProgress` остаётся валидным алиасом через `pub use`.
- `api/src/router.rs::tests::server` (line 338) — `progress: std::sync::Arc::new(dashmap::DashMap::new())` — без изменений.
- `app/src/container.rs:50,82,178` (`progress: LiveProgress`, инициализация `Arc::new(dashmap::DashMap::new())`) — без изменений.

### Контрольная сборка

- `cargo test --manifest-path rust/Cargo.toml` — полный зелёный регресс.
  Особенно важно: `api/src/routes/jobs.rs` (`active_lists_only_in_flight_jobs_with_merged_progress`,
  `status_merges_live_progress_and_persisted_error_class`) и
  `api/src/router.rs::tests` (`all_seven_api_routes_*`).
- `cargo build --manifest-path rust/Cargo.toml --bin meeting-server` — должна
  пройти.
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`
  если в репо ходят с clippy-deny — иначе ничего страшного.

## Functional Requirements

1. **Одна точка истины.** `grep -rn 'pub type LiveProgress' rust/crates/`
   возвращает ровно одну строку — в `core/src/live.rs`.
2. **Внешний API не изменился.** `meeting_api::LiveProgress`,
   `meeting_adapters::LiveProgress`, `meeting_adapters::worker::LiveProgress`,
   `meeting_api::router::LiveProgress` — все валидные пути, разрешающиеся
   в один и тот же тип.
3. **Поведения нет:** ноль изменений в `GET /api/v1/jobs/:id`,
   `GET /api/v1/active-jobs`, в writer-логике `Worker::execute`,
   в `JobProgress` payload.

## Edge Cases

- **`core` без `serde`-фичи `dashmap`.** Не нужна — `DashMap` сериализуется
  через итерацию в `api/routes/jobs.rs` (handler собирает `Vec<JobResponse>`);
  в core ничего не сериализуется.
- **`fakes` feature.** `LiveProgress` не зависит от `fakes` — type alias
  всегда доступен. Существующие fake-конструкции
  `Arc::new(DashMap::new())` в тестах остаются валидными.
- **Циклическая зависимость.** Нет: `core` ещё не зависит от `dashmap`,
  добавление не создаёт циклов (`adapters`/`api` уже зависят от `core`
  и от `dashmap` независимо через workspace).
- **`meeting-server.rs` binary.** Использует `LiveProgress` только через
  `container.rs` и `AppState` — не правится.

## Acceptance Criteria

- [ ] `pub type LiveProgress = ...` определён ровно один раз
      (`rust/crates/core/src/live.rs`).
- [ ] `meeting_core::LiveProgress` экспортируется из `core/src/lib.rs`.
- [ ] `meeting-api::router::LiveProgress` и
      `meeting-adapters::worker::LiveProgress` — это
      `pub use meeting_core::LiveProgress`.
- [ ] `app/tests/live_progress_unified.rs` компилируется в режиме тестов
      (`cargo test -p meeting-app --no-run` зелёный) — функция, передающая
      `meeting_api::LiveProgress` как `meeting_adapters::LiveProgress`,
      компилируется.
- [ ] `cargo test --manifest-path rust/Cargo.toml` весь зелёный.
- [ ] `cargo build --manifest-path rust/Cargo.toml --bin meeting-server`
      проходит.
- [ ] Нет изменений в `qt-app/`, `prompts/`, `rust/migrations/`.

## Implementation Phases

1. **Core.** Создать `core/src/live.rs`, добавить `mod live` + `pub use`
   в `lib.rs`, добавить `dashmap` в `core/Cargo.toml`.
   `cargo build -p meeting-core` зелёный.
2. **Re-export.** Заменить локальные `pub type LiveProgress` на
   `pub use meeting_core::LiveProgress` в `api/router.rs` и
   `adapters/worker.rs`. Грепнуть `DashMap` импорты в этих файлах и
   снять unused.
3. **Compile-проверка.** Добавить `app/tests/live_progress_unified.rs`.
4. **Регресс.** `cargo test --manifest-path rust/Cargo.toml`.
5. **Closing step.** Сделать **closing commit**, который выполняет
   `git mv plans/active/live-progress-unification plans/done/live-progress-unification`.

## Depends On / Related

- [[worker-concurrency-pool]] — active; не блокирует и не блокируется этой
  задачей. В его PRD уже отмечено: «live-progress-unification —
  рекомендуемый, но не блокирующий пререкизит».
- [[job-progress-sse]], [[job-cancellation]] — следующие потребители
  единого `LiveProgress`; решение «alias → newtype» можно поднять в их
  PRD, не в этой задаче.
