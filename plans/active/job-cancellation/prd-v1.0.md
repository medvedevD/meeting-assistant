# Job Cancellation

## Problem

Зависшую job (огромное аудио, медленный LLM-провайдер, runaway whisper) можно
остановить только убийством сидекара — это теряет crash-safe state, рушит
другие jobs из общего пула и заставляет пользователя ждать рестарта. В UI
аффорданса «Отмена» тоже нет ([backlog/job-cancellation.md](../../../backlog/job-cancellation.md)).
Текущий [`Worker::execute`](../../../rust/crates/adapters/src/worker.rs#L176)
не имеет точек кооперативной остановки: `transcribe_with_progress` и
`generate_protocol` запускаются как `await` без `select!` против отмены,
а Whisper-инференс ([`run_whisper`](../../../rust/crates/adapters/src/whisper.rs#L103))
крутится в `spawn_blocking` без `abort_callback`.

## Goal

Кооперативная отмена конкретной job:

- `DELETE /api/v1/jobs/:id` останавливает job на ближайшем безопасном чекпоинте
  без побочных эффектов на другие jobs и сам процесс сидекара.
- Терминальное состояние явно: `status='failed'`, `error_class='cancelled'`.
- Цепочка `then_protocol` пропускается, если транскрипция была отменена.
- UI получает кнопку «Прервать» на `MeetingDetailScreen` и ✕ в списке
  активных jobs в `AppShell`.

## Expected Outcome

| Сценарий | Сегодня | После |
|---|---|---|
| Зависшая транскрипция (часовое аудио) | kill сайдкара, потеря всех jobs | DELETE /jobs/:id → терминал ≤30 с (один сегмент Whisper) |
| Медленный LLM (большой транскрипт) | kill сайдкара | DELETE /jobs/:id → терминал немедленно (на `select!`) |
| Отмена pending job из очереди | нет способа | DELETE /jobs/:id → 202, status=failed моментально |
| Двойной DELETE | undefined | первый 202, второй 204, без race |
| Краш сайдкара во время cancel | running остаётся | `recover_running_jobs` → pending, пользователь повторяет cancel |

## Scope

**В этой задаче:**

- Расширение `LiveJobs` (бывший `LiveProgress`): значение становится составным
  `LiveEntry { progress, cancel }`, токен и прогресс живут одной записью.
- Новые методы `JobRepo::cancel_pending` и `JobRepo::mark_cancelled`,
  идемпотентные через `WHERE status IN ('pending','running')`.
- Use-case `cancel_job(repo, live, id)` с тремя ветвями: `Cancelled`,
  `Cancelling`, `AlreadyTerminal`.
- Роут `DELETE /api/v1/jobs/:id` (202 / 204 / 404), под общим bearer-гейтом.
- Чекпоинты в `Worker::execute`: на каждом `set_stage` + `tokio::select!`
  вокруг LLM-вызова; пропуск ветки `then_protocol` при отмене.
- Whisper abort: `set_abort_callback_safe` с проброшенным токеном — проверка
  между сегментами (≤30 c гранулярность).
- `ErrorClass::Cancelled` + рендер «Отменено» в `PipelineProgress.qml`.
- `ActiveJobsStore.cancel(meetingId)`, кнопка на `MeetingDetailScreen`,
  ✕ в списке активных в `AppShell`.

**Вне этой задачи:**

- Отмена записи (`POST /api/v1/recordings/:id/stop` уже есть).
- SSE для подтверждения отмены ([`backlog/job-progress-sse.md`](../../../backlog/job-progress-sse.md)) —
  существующий polling 1 с достаточен.
- Отмена цепочки уже-enqueued protocol-job, если транскрипция успела
  finished'нуть до cancel'а — отдельный DELETE по id цепной job'ы.
- Batch-cancel («отменить все активные») — отдельный backlog при появлении сценария.
- Partial-write cleanup транскрипта/протокола при отмене на середине
  `write_transcript` — `fs::write` атомарен на уровне файла, partial-state
  следующий reprocess перезаписывает. Acceptable.

## Verified prerequisites

- **`tokio_util::sync::CancellationToken`** — синхронный `is_cancelled()` для
  FFI-колбэка Whisper и асинхронный `cancelled().await` для `tokio::select!`.
  `tokio` уже в зависимостях; `tokio-util` добавляется в `meeting-core` и
  `meeting-adapters` (feature-flag не нужен — `CancellationToken` в default).
- **whisper-rs 0.16 `set_abort_callback_safe`** — сигнатура `FnMut() -> bool`,
  вызывается между сегментами ([whisper-rs whisper_params.rs:621](https://docs.rs/whisper-rs/0.16.0/whisper_rs/struct.FullParams.html#method.set_abort_callback_safe)).
  Уже доступен через текущую версию ([rust/Cargo.lock](../../../rust/Cargo.lock)).
- **`ApiClient::del` уже есть** ([qt-app/src/ApiClient.h:35](../../../qt-app/src/ApiClient.h#L35))
  и проксирован в [qt-app/qml/Request.qml:34](../../../qt-app/qml/Request.qml#L34) —
  UI-сторона не требует C++ изменений.
- **`LiveProgress` единый тип через `meeting-core`** ([rust/crates/core/src/live.rs:8](../../../rust/crates/core/src/live.rs#L8)),
  все потребители (`api`, `adapters`, `app`) re-export'ят его. Переименование
  `LiveProgress` → `LiveJobs` точечное (≤8 файлов).
- **`Worker` уже спавнит per-job task через `JoinSet`** ([adapters/src/worker.rs:149](../../../rust/crates/adapters/src/worker.rs#L149))
  — токен можно вставить ровно перед `spawn`, без рефакторинга concurrency
  модели.

## Decisions

1. **Примитив отмены — `CancellationToken`.** Sync-readable из Whisper FFI и
   async-awaitable из `tokio::select!` одним объектом. Альтернативы:
   `Arc<AtomicBool>` (нет `.await`), `tokio::sync::Notify` (single-shot, не
   идемпотентен на multiple readers).
   - *Статус:* твёрдо.

2. **Хранилище — один map, составное значение.** `LiveJobs = Arc<DashMap<String,
   LiveEntry>>`, где `LiveEntry { progress: JobProgress, cancel: CancellationToken }`.
   Не вводим вторую `Cancellations` map — иначе drift между двумя записями
   при insert/remove. Лайфтайм у обоих полей идентичен (создаются при claim,
   удаляются при `clear_progress`).
   - *Почему не два map'а:* единый insert/remove исключает рассинхрон;
     `set_stage` мутирует `.progress` in-place, оставляя `cancel` живым.
   - *Статус:* твёрдо.

3. **Терминал — отдельный `mark_cancelled`, не `mark_permanently_failed`.**
   Reuse `mark_permanently_failed` с `attempts=MAX_ATTEMPTS, error_class='cancelled'`
   врёт про счётчик попыток (телеметрия теряется). `mark_cancelled` сохраняет
   semantic split «юзер прервал» vs «исчерпались retries».
   - *Статус:* твёрдо.

4. **HTTP-контракт: 202 / 204 / 404, тело пустое.**
   | pre-state | response | действие репо |
   |---|---|---|
   | not found | 404 | — |
   | `pending` | 202 | `cancel_pending` (мгновенный терминал) |
   | `running` | 202 | `live.get(id).cancel.cancel()`; терминал ставит воркер |
   | `done`/`failed` | 204 | — |
   - *Почему пустое тело:* UI уже polling'ует `/jobs/:id` и `/active-jobs`;
     finite state приходит через них, отдельный канал не нужен.
   - *Статус:* твёрдо.

5. **Permit-and-token before spawn.** Токен создаётся и вставляется в
   `LiveJobs` **между** успешным `claim_pending_kind` и `inflight.spawn`,
   синхронно с инсертом первого `JobProgress`. Cancel, прилетевший в окно
   между claim и insert (несколько микросекунд), увидит `running` в DB без
   live-entry — обрабатывается как «running без токена → 202 best-effort,
   токен ещё не появился, следующий progress-tick покажет реальный статус».
   - *Статус:* твёрдо.

6. **Whisper отмена — unsafe FFI `set_abort_callback` + `Arc<AbortFlag>`, не
   `set_abort_callback_safe`.** Безопасный wrapper в whisper-rs 0.16 имеет
   **два бага** (см. [ADR-005](#adr-005)):
   (1) trampoline кастит `user_data` как `*mut F`, но фактически в нём лежит
   `*mut Box<dyn FnMut() -> bool>` — несовместимые memory layouts → UB на
   каждом вызове; (2) trampoline берёт `&mut *user_data` без синхронизации,
   при этом whisper.cpp вызывает callback **из множества inference-thread'ов
   параллельно** → aliased mutable references + data race → futex-deadlock.

   Текущее решение: собственный `extern "C" fn abort_cb(*mut c_void) -> bool`,
   `user_data` = указатель в стабильный `Arc<AbortFlag>`, где
   `AbortFlag { cancelled: AtomicBool }`. Callback делает **только**
   `AtomicBool::load(Relaxed)` — никаких `&mut`, никаких captures, никаких
   FFI-mismatch'ей; параллельные вызовы безопасны by construction.

   Связь `CancellationToken → AbortFlag` через watcher-таску (хелпер
   `with_abort_watcher`): спавнит `tokio::select!` на `token.cancelled()`,
   флипает `AtomicBool` при отмене; watcher join'ится **до** drop'а Arc'а,
   так что raw pointer валиден всё время выполнения `state.full`.

   Гранулярность отмены — ≤30 c (между сегментами whisper.cpp).
   - *Статус:* твёрдо.

7. **`CoreError::Cancelled` — отдельный вариант, не `Transcription("cancelled")`.**
   `handle_failure` должен сразу различать «retry candidate» и «terminal cancel»
   без парсинга строки. `from_core_error` для нового варианта возвращает
   `ErrorClass::Cancelled` (детерминированный маппинг для симметрии с
   остальными классами).
   - *Статус:* твёрдо.

8. **`ErrorClass::Cancelled` — `"cancelled"` в SQL.** Колонка `error_class`
   уже `TEXT NULL` ([db/job_repo.rs:37](../../../rust/crates/adapters/src/db/job_repo.rs#L37)),
   миграция не нужна.
   - *Статус:* твёрдо.

9. **Token cleanup в `clear_progress`.** `LiveEntry` целиком дропается;
   уже-удерживаемый клон токена у вызывающего (Whisper-FFI, идущая select-arm)
   продолжает работать. На терминале это no-op — `is_cancelled()` будет
   возвращать prev state (false, если cancel не вызывался; true, если вызывался).
   - *Статус:* твёрдо.

10. **Краш сайдкара после `.cancel()` — задокументированное поведение.**
    Токен только в памяти; `recover_running_jobs` ([db/job_repo.rs:237](../../../rust/crates/adapters/src/db/job_repo.rs#L237))
    вернёт row в `pending`, пользователь повторно нажмёт «Прервать».
    Альтернатива (персистить `cancel_requested` флаг в DB) — overkill, окно
    редкое, retry-cost нулевой.
    - *Статус:* твёрдо.

## ADR-005

### Mid-inference Whisper abort через unsafe FFI

**Status:** Accepted (заменяет первоначальный план Phase 5).

**Context.** Phase 5 первой итерации использовал
`params.set_abort_callback_safe(move || cancel.is_cancelled())`. Это
дедлокит whisper.cpp на первом же вызове: все inference-thread'ы уходят в
`futex_wait`, `state.full` никогда не возвращается. Корень — два бага в
whisper-rs 0.16:

1. **Type mismatch**: `set_abort_callback_safe` хранит closure как
   `Box<Box<dyn FnMut() -> bool>>` (Box-of-Box, thin pointer to fat pointer),
   а trampoline кастит `user_data as *mut F` — несовместимая memory layout.
2. **Aliased mutable access**: trampoline берёт `&mut *user_data` без
   синхронизации, при том что whisper.cpp вызывает callback из множества
   inference-thread'ов параллельно — data race на `FnMut`, UB по-крупному.

Сравните с `set_progress_callback_safe`: тот же broken design, но whisper.cpp
вызывает его из одного main thread, поэтому race не материализуется. У
abort_callback — материализуется как наш futex-deadlock.

**Decision.** Использовать unsafe `set_abort_callback` + `set_abort_callback_user_data`
с собственным `extern "C"` trampoline и `Arc<AbortFlag>` как user_data.
`AbortFlag` содержит только `AtomicBool` — никаких closures, никаких captures,
никаких vtable'ей. Callback делает один `AtomicBool::load(Relaxed)`. Параллельные
вызовы из inference-thread'ов whisper.cpp безопасны by construction.

Связь с `CancellationToken` через хелпер `with_abort_watcher(token, f)`:
- спавнит `tokio::select! { token.cancelled() => flag.store(true); stop_rx => () }`;
- запускает `f(Arc<AbortFlag>)` в `spawn_blocking` (там `f` зовёт `run_whisper`);
- по выходе из `spawn_blocking` сигналит stop_rx, join'ит watcher;
- `drop(Arc)` происходит строго **после** join — указатель валиден всё
  время `state.full`.

**Options considered.**

1. **Оставить kill-switch (post-call only)** — UX неприемлем: на часовом
   аудио пользователь ждёт ~час между «Прервать» и фактическим терминалом.
2. **Update whisper-rs** — в 0.16.0 фикса нет; trunk не гарантирует фикс
   (race требует серьёзного редизайна обёртки).
3. **Fork whisper-rs** — maintenance debt вечно.
4. **Unsafe FFI + Arc<AbortFlag>** ⭐ — выбрано: ~50 строк, изолировано, by
   construction корректно.
5. **Tokio AbortHandle на `spawn_blocking`** — невозможно архитектурно
   (Tokio не умеет прерывать blocking threads).
6. **Inference в child-процессе + SIGKILL** — 100× сложнее, требует IPC.

**Consequences.**

➕ Mid-inference прерывание реально работает — гранулярность ≤30 c (между
сегментами whisper).
➕ Чистый contract: `AbortFlag` без полей-сюрпризов, lifetime инвариант
формальный (Arc drop'ается строго после `state.full`).
➕ Один `unsafe { ... }` блок, документированный SAFETY-комментарием.
➕ Тред-безопасность доказуема: единственная операция — atomic load.

➖ `unsafe` — code review должен проверять lifetime Arc'а каждый раз, когда
кто-то редактирует `with_abort_watcher` или его call-sites.
➖ Если whisper-rs когда-нибудь починят safe-wrapper и пометят unsafe
`set_abort_callback` как deprecated — нам потребуется минор-рефактор. Маловероятно
быстро.

## Deliverables

### Backend

**1. `rust/crates/core/Cargo.toml`** — добавить `tokio-util = { version = "0.7", default-features = false }`
(нужно только `CancellationToken` из default-set'а).

**2. `rust/crates/core/src/live.rs`** — переписать:

```rust
use crate::entities::JobProgress;
use dashmap::DashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct LiveEntry {
    pub progress: JobProgress,
    pub cancel: CancellationToken,
}

pub type LiveJobs = Arc<DashMap<String, LiveEntry>>;
```

Re-export'ы переименовать: `meeting_core::LiveProgress` → `meeting_core::LiveJobs`
(и одноимённые в `meeting_api`, `meeting_adapters`, `meeting_app`).

**3. `rust/crates/core/src/entities/job.rs`** — расширить `ErrorClass`:

```rust
pub enum ErrorClass { /* existing */, Cancelled }
// as_str/from_str: "cancelled"
// from_core_error: CoreError::Cancelled => Self::Cancelled
```

**4. `rust/crates/core/src/lib.rs`** — добавить `CoreError::Cancelled` (unit-вариант,
без payload — отмена это состояние, не сообщение):

```rust
#[error("cancelled by user")]
Cancelled,
```

**5. `rust/crates/core/src/ports/job_repo.rs`** — два новых метода:

```rust
/// Атомарно: pending → failed(cancelled). Возвращает rows_changed.
/// Идемпотентно: на не-pending row возвращает 0.
async fn cancel_pending(&self, id: &str, now_ts: i64) -> Result<u64, CoreError>;

/// Помечает running job как failed(cancelled) после того как воркер
/// наблюдал cancellation token. Guard: `status IN ('pending','running')`,
/// чтобы быть no-op'ом против гонки с `mark_done`.
async fn mark_cancelled(&self, id: &str, now_ts: i64) -> Result<u64, CoreError>;
```

**6. `rust/crates/core/src/usecases/cancel_job.rs`** — новый файл:

```rust
pub enum CancelOutcome { Cancelling, Cancelled, AlreadyTerminal, NotFound }

pub async fn cancel_job(
    repo: Arc<dyn JobRepo>,
    live: LiveJobs,
    id: &str,
) -> Result<CancelOutcome, CoreError> {
    let Some(job) = repo.find_by_id(id).await? else {
        return Ok(CancelOutcome::NotFound);
    };
    match job.status {
        JobStatus::Pending => {
            let n = repo.cancel_pending(id, now_unix()).await?;
            Ok(if n == 0 { CancelOutcome::AlreadyTerminal }
               else      { CancelOutcome::Cancelled })
        }
        JobStatus::Running => {
            if let Some(entry) = live.get(id) { entry.cancel.cancel(); }
            // best-effort: окно claim↔insert — нет токена ≠ нет cancel'а;
            // воркер проверит токен на следующем чекпоинте через DB-флаг
            // (см. Decision #5: окно микросекундное, retry безопасен).
            Ok(CancelOutcome::Cancelling)
        }
        JobStatus::Done | JobStatus::Failed => Ok(CancelOutcome::AlreadyTerminal),
    }
}
```

Зарегистрировать в [`usecases/mod.rs`](../../../rust/crates/core/src/usecases/mod.rs).

**7. `rust/crates/adapters/src/db/job_repo.rs`** — реализация:

```rust
async fn cancel_pending(&self, id: &str, now_ts: i64) -> Result<u64, CoreError> {
    // UPDATE jobs SET status='failed', error_class='cancelled',
    //                last_error='cancelled by user', updated_at=?1
    //  WHERE id=?2 AND status='pending'
}

async fn mark_cancelled(&self, id: &str, now_ts: i64) -> Result<u64, CoreError> {
    // UPDATE jobs SET status='failed', error_class='cancelled',
    //                last_error='cancelled by user', updated_at=?1
    //  WHERE id=?2 AND status IN ('pending','running')
}
```

`spawn_blocking` обёртка как у остальных методов.

**8. `rust/crates/core/src/fakes.rs`** — обновить `FakeJobRepo`:

- `cancel_pending`: проверка `status == Pending`, иначе 0; на успех
  `status = Failed`, `error_class = Cancelled`.
- `mark_cancelled`: проверка `status ∈ {Pending, Running}`.

**9. `rust/crates/api/src/routes/jobs.rs`** — handler:

```rust
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    use meeting_core::usecases::CancelOutcome::*;
    match cancel_job(Arc::clone(&state.job_repo), Arc::clone(&state.progress), &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        NotFound => Err((StatusCode::NOT_FOUND, format!("job {id} not found"))),
        Cancelling | Cancelled => Ok(StatusCode::ACCEPTED),
        AlreadyTerminal => Ok(StatusCode::NO_CONTENT),
    }
}
```

**10. `rust/crates/api/src/router.rs`** — регистрация:

- [`api_routes`](../../../rust/crates/api/src/router.rs#L133): заменить
  `.route("/api/v1/jobs/:id", get(jobs::status))` на
  `.route("/api/v1/jobs/:id", get(jobs::status).delete(jobs::cancel))`.
- [`API_ROUTES`](../../../rust/crates/api/src/router.rs#L345) — добавить
  `("DELETE", "/api/v1/jobs/abc")` в auth-coverage список.

**11. `rust/crates/adapters/src/worker.rs`** — интеграция токена:

- Сразу после `claim_pending_kind` returns `Some(job)`, **до** `spawn`:
  ```rust
  let cancel = tokio_util::sync::CancellationToken::new();
  self.progress.insert(job.id.clone(), LiveEntry {
      progress: JobProgress::new(PipelineStage::Queued, "В очереди", 0),
      cancel: cancel.clone(),
  });
  let me = Arc::clone(&self);
  inflight.spawn(async move {
      me.execute(job, cancel).await;
      drop(permit);
  });
  ```
- `set_stage(&self, job_id, stage)` теперь мутирует `.progress` через
  `progress.alter` / `progress.entry().and_modify()`, не overwrites
  `LiveEntry` (иначе теряется токен). Аналогично `transcribe_sink`.
- `execute(&self, job, cancel: CancellationToken)`:
  ```rust
  if cancel.is_cancelled() {
      let _ = self.job_repo.mark_cancelled(&job.id, now_unix()).await;
      self.clear_progress(&job.id);
      return;
  }
  let result = if job.kind.is_transcription() {
      self.run_transcribe(&job, &meeting, cancel.clone()).await
  } else {
      self.run_regenerate_protocol(&job, &meeting, cancel.clone()).await
  };
  match result {
      Ok(()) if !cancel.is_cancelled() => {
          // существующая ветка: then_protocol + mark_done
      }
      Ok(()) | Err(CoreError::Cancelled) => {
          // succeded but cancelled in-window, ИЛИ explicit cancel error
          let _ = self.job_repo.mark_cancelled(&job.id, now_unix()).await;
          self.clear_progress(&job.id);
          // НЕ enqueue then_protocol
      }
      Err(e) => self.handle_failure(&job, &e).await,
  }
  ```
- `run_transcribe`/`run_regenerate_protocol` принимают `CancellationToken`,
  пробрасывают в `Transcriber::transcribe_with_progress` (новый параметр)
  и оборачивают LLM-вызов:
  ```rust
  tokio::select! {
      biased;
      _ = cancel.cancelled() => Err(CoreError::Cancelled),
      r = generate_protocol(...) => r,
  }
  ```

**12. `rust/crates/core/src/ports/transcriber.rs`** — расширить port:

```rust
async fn transcribe_with_progress(
    &self,
    audio_path: &Path,
    on_progress: ProgressSink,
    _cancel: CancellationToken,   // default: ignore
) -> Result<Transcript, CoreError> {
    self.transcribe(audio_path).await
}
```

Дефолт игнорирует токен — фейки и простые адаптеры остаются без изменений.

**13. `rust/crates/adapters/src/whisper.rs`** — wire abort:

- `RealWhisperRunner::run` принимает `cancel: CancellationToken`, кладёт его
  в `spawn_blocking` замыкание. В `run_whisper`:
  ```rust
  let cancel_for_abort = cancel.clone();
  params.set_abort_callback_safe(move || cancel_for_abort.is_cancelled());
  ```
- После `state.full(...)`: если `cancel.is_cancelled()`, вернуть
  `CoreError::Cancelled` явно (whisper.cpp на abort возвращает `Err`,
  который мапится в `Transcription("aborted")` — нужно перехватить и
  переклассифицировать).
- Синтетический ETA-ticker завершается как обычно (на `tx_stop.send`).

### UI

**14. `qt-app/qml/ActiveJobsStore.qml`** — новая функция:

```qml
function cancel(meetingId) {
    var e = _jobs[meetingId]
    if (!e || e.terminalAt !== 0) return
    var req = _reqComp.createObject(store)
    if (req === null) return
    req.ok.connect(function () { req.destroy() })
    req.fail.connect(function (s, err) { req.destroy() })
    req.del("/api/v1/jobs/" + e.jobId)
    e.cancelRequested = true       // оптимистичный UI-хинт
    _bump()
}
```

`_bump()` уже есть как способ инкрементировать `version`.

**15. `qt-app/qml/screens/MeetingDetailScreen.qml`** — кнопка «Прервать»:

- Видима, когда `ActiveJobsStore.isActive(meeting.id)`.
- По клику — `MeetyConfirmDialog` («Прервать обработку?» / «Прервать» /
  «Отмена»), на confirm — `ActiveJobsStore.cancel(meeting.id)`.
- Скрыта (или disabled), когда `entry.cancelRequested === true` — показывает
  «Прерывание…» через короткий transient state.

**16. `qt-app/qml/AppShell.qml`** ([:675](../../../qt-app/qml/AppShell.qml#L675))
— список активных:

- В delegate row'а — `ToolButton` с ✕ icon, тот же confirm-flow.

**17. `qt-app/qml/components/PipelineProgress.qml`** — рендер терминала:

- Когда `job.error_class === "cancelled"` — показать «Отменено» (нейтральный
  стиль, не error banner). Отличается визуально от `api_auth`/`audio_corrupt`
  etc., которые остаются красными.

### Тесты

**1. `rust/crates/core/src/usecases/cancel_job.rs` (unit):**

- `cancel_unknown_returns_not_found`.
- `cancel_pending_returns_cancelled_and_persists_state`: после вызова
  `find_by_id` показывает `status=failed, error_class=cancelled`.
- `cancel_running_returns_cancelling_and_triggers_token`: вставить
  `LiveEntry` в map с токеном, после вызова `entry.cancel.is_cancelled() == true`.
- `cancel_already_terminal_returns_already_terminal`.
- `double_cancel_idempotent`: второй вызов на pending → AlreadyTerminal
  (`cancel_pending` возвращает 0 rows).

**2. `rust/crates/adapters/src/db/job_repo.rs` (unit):**

- `cancel_pending_marks_failed_with_cancelled_class`.
- `cancel_pending_noop_on_running` (rows_changed == 0).
- `mark_cancelled_marks_failed_from_running`.
- `mark_cancelled_noop_on_done`.

**3. `rust/crates/adapters/src/worker.rs` (unit, расширяя существующие):**

- `cancel_pending_inflight_does_not_chain_then_protocol`: enqueue T с
  `then_protocol=true`; `GatedTranscriber` блокирует; cancel.cancel();
  release gate → `mark_cancelled` сработал, `list_active()` пуст
  (нет цепной protocol-job'ы).
- `cancel_during_protocol_marks_cancelled_not_failed`: P-job на медленном
  `FakeLlmProvider` (с Notify), cancel → терминал `cancelled`, не
  retry candidate.
- `cancel_after_done_is_noop`: job уже `Done`; cancel.cancel() на токене
  (если ещё в map'е) — `execute` уже вышел, ничего не падает.

**4. `rust/crates/api/src/routes/jobs.rs` (unit):**

- `delete_unknown_returns_404`.
- `delete_pending_returns_202`.
- `delete_done_returns_204`.
- `delete_running_returns_202_and_sets_token`: сидим в `LiveJobs` руками,
  после DELETE → `entry.cancel.is_cancelled()`.

**5. `rust/crates/api/src/router.rs`** — расширить
[`all_seven_api_routes_401_without_token`](../../../rust/crates/api/src/router.rs#L373)
кейсом `("DELETE", "/api/v1/jobs/abc")` в `API_ROUTES`.

**6. `rust/crates/app/tests/`** — интеграционный smoke:

- `cancel_running_transcribe_via_http`: поднять полный server с
  `LazyWhisperTranscriber` (или fake), отправить транскрипцию, через 100 мс
  DELETE → job переходит в `failed/cancelled` в пределах 1 с.

### QML-тесты

[qt-app/tests/](../../../qt-app/tests/) — добавить (или расширить
существующий pipeline-progress test):

- `active_jobs_store_cancel_calls_delete`: мок Request, проверить вызов
  `del("/api/v1/jobs/" + jobId)`.
- `pipeline_progress_renders_cancelled_neutral`: подать job с
  `error_class="cancelled"`, проверить отсутствие красного error-banner'а.

## Functional Requirements

1. **Cancel pending → терминал моментально.** DELETE на pending job
   возвращает 202; следующий GET показывает `status=failed,
   error_class=cancelled` в пределах одного round-trip'а.

2. **Cancel running transcribe → терминал ≤30 с.** DELETE на running
   транскрипцию останавливает Whisper на границе следующего сегмента,
   воркер выходит из `execute` с `CoreError::Cancelled`, `mark_cancelled`
   персистится, прогресс-entry удаляется.

3. **Cancel running protocol → терминал немедленно.** DELETE на running
   protocol-job прерывает LLM-вызов через `tokio::select!`, терминал ставится
   в пределах одного poll'а (1 с).

4. **Цепочка then_protocol пропускается.** Отменённая транскрипция с
   `then_protocol=true` не enqueue'ит protocol-job (test:
   `cancel_pending_inflight_does_not_chain_then_protocol`).

5. **Идемпотентность.** Двойной DELETE: первый 202, второй 204; никаких
   повторных state changes.

6. **Изоляция между jobs.** Cancel одной job не влияет на остальные jobs
   в обоих пулах (`Transcribe` и `Protocol`); они продолжают исполняться.

7. **UI affordances.** Кнопка «Прервать» на `MeetingDetailScreen` видима
   iff `ActiveJobsStore.isActive(meeting.id)`; ✕ в активном списке;
   confirm-dialog предотвращает случайные отмены.

8. **Терминальная индикация.** `PipelineProgress` для отменённой job
   показывает «Отменено» в нейтральном стиле, не как ошибку.

## Edge Cases

- **Cancel в окне `claim_pending_kind` ↔ `LiveJobs.insert`.** DB показывает
  `running`, live-entry ещё нет. `cancel_job` возвращает `Cancelling`
  (best-effort), но cancel не дошёл до воркера. Worker сразу после insert'а
  начнёт работу. Поведение: пользователь увидит, что отмена «не сработала»,
  пошлёт повторный DELETE через несколько секунд — этот уже попадёт в
  `entry.cancel.cancel()`. Окно микросекундное; задокументированное.

- **Cancel between `Ok(())` и `mark_done`.** Job только что закончилась,
  cancel пришёл в эту микросекунду. `cancel.cancelled()` уже true, но
  `execute` уже в ветке успеха. Логика: проверяем `cancel.is_cancelled()`
  после `Ok(())` (см. Deliverable 11) — если true, `mark_cancelled` вместо
  `mark_done`, цепочка пропускается. Поведение: пользователь получил то,
  что просил (отмену) даже на грани.

- **Cancel running protocol с уже-частично-записанным `protocol.md`.**
  `write_protocol` — single `fs::write`; если cancel прилетел до неё, файл
  не пишется; если после — файл записан, но job всё равно
  `failed/cancelled`. UI следующего раза покажет «есть протокол, но job
  отменён» — кнопка «Перегенерировать» решает. Acceptable.

- **Cancel running transcribe.** Whisper-инференс прерывается между
  сегментами через unsafe FFI `set_abort_callback` (см. Decision #6 и
  [ADR-005](#adr-005)). Гранулярность — длительность одного сегмента
  whisper.cpp (≤30 c). Job помечается терминалом `cancelled`, цепочка
  `then_protocol` пропускается.

- **Сидекар крашится сразу после `entry.cancel.cancel()`.** Токен в памяти
  потерян; `recover_running_jobs` вернёт job в `pending`. Пользователь
  при следующем старте увидит «Прерванную» job снова в очереди — может
  повторно нажать «Прервать» или дать выполниться. Документированное.

- **DELETE на recovered-from-running job (только что переведённую в
  `pending` через `recover_running_jobs`).** Standard `cancel_pending` path
  работает без особенностей.

- **`POLL_INTERVAL` sleep с уже-cancelled токеном.** Воркер sleep'ает на
  `Ok(None)` после claim'а — token этой job'ы уже мог быть отменён, но
  воркер этого не увидит, потому что job не в его `inflight` set'е (она
  уже была успешно cancel'ом сделана `failed`). Цикл claim'а на следующий
  тик увидит updated DB state. Нет race.

## Acceptance Criteria

- [ ] `tokio-util` добавлен в `meeting-core/Cargo.toml`, `CancellationToken`
      использован в `LiveEntry`.
- [ ] `LiveProgress` переименован в `LiveJobs` во всех re-export'ах; все
      reader'ы (`api::routes::jobs`, `app::container`) обновлены на
      `.value().progress.clone()`.
- [ ] `CoreError::Cancelled` и `ErrorClass::Cancelled` добавлены; маппинг
      `from_core_error` симметричен.
- [ ] `JobRepo::cancel_pending` и `mark_cancelled` реализованы в
      `SqliteJobRepo` и `FakeJobRepo`, оба идемпотентны через `WHERE
      status IN (...)`.
- [ ] Usecase `cancel_job` экспортирован из `meeting_core::usecases`.
- [ ] `DELETE /api/v1/jobs/:id` зарегистрирован, под bearer-гейтом,
      добавлен в `API_ROUTES` auth-coverage тест.
- [ ] `Worker` пробрасывает `CancellationToken` в `execute`, оба
      `run_transcribe`/`run_regenerate_protocol` чекпоинтят токен, цепочка
      `then_protocol` пропускается на отмене.
- [ ] `LazyWhisperTranscriber::transcribe_with_progress` принимает
      `CancellationToken`, выставляет `set_abort_callback_safe`,
      переклассифицирует abort-error в `CoreError::Cancelled`.
- [ ] Юнит-тесты `cancel_job` (5 кейсов) зелёные.
- [ ] Юнит-тесты `db/job_repo.rs` cancel-кейсы (4 кейса) зелёные.
- [ ] Юнит-тесты воркера cancel-кейсы (3 кейса) зелёные.
- [ ] Юнит-тесты `routes/jobs.rs` DELETE (4 кейса) зелёные.
- [ ] Интеграционный smoke `cancel_running_transcribe_via_http` зелёный.
- [ ] QML: `ActiveJobsStore.cancel(meetingId)`, кнопка «Прервать» на
      `MeetingDetailScreen`, ✕ в списке активных, рендер «Отменено» в
      `PipelineProgress`.
- [ ] Ручная проверка: запустить транскрипцию часового аудио, нажать
      «Прервать» — UI показывает «Отменено» в течение 30 с, сайдкар
      продолжает обслуживать другие jobs (если они есть).

## Implementation Phases

1. **Core-типы и trait.** `LiveJobs`/`LiveEntry`, `CoreError::Cancelled`,
   `ErrorClass::Cancelled`, новые методы `JobRepo` (+ fakes), usecase
   `cancel_job`. Обновить все call-site'ы `LiveProgress` → `LiveJobs` с
   распаковкой `.progress`. Поведения нет, но компилируется зелёным.

2. **SQLite репо.** Реализация `cancel_pending` и `mark_cancelled` +
   юнит-тесты. Зелёный регресс.

3. **API роут.** `DELETE /api/v1/jobs/:id`, регистрация, `API_ROUTES`
   расширение, тесты handler'а. Фича доступна, но без cooperative
   cancellation в воркере (cancel pending работает; running остаётся «как
   есть» до собственного завершения).

4. **Worker интеграция.** Токен при claim, чекпоинты, `select!` вокруг
   LLM, пропуск `then_protocol`. Воркерные тесты concurrent + cancel.

5. **Whisper abort.** `set_abort_callback_safe` + reclassify abort-error.
   Smoke-тест на полной транскрипции (можно скипать в CI, гнать локально).

6. **UI.** `ActiveJobsStore.cancel`, MeetingDetailScreen кнопка,
   AppShell ✕, PipelineProgress «Отменено». QML-тесты.

7. **Closing step.** Closing-commit с `git mv plans/active/job-cancellation
   plans/done/job-cancellation` и заметкой в [backlog/job-cancellation.md](../../../backlog/job-cancellation.md)
   о доставке.

## Depends On / Related

- [[worker-concurrency-pool]] — построилось двух-пуловое разделение, на
  котором cancel становится per-job операцией без cross-pool side effects.
- [[live-progress-unification]] — `LiveProgress` уже единый тип в
  `meeting-core`, переименование в `LiveJobs` точечно.
- [[job-progress-sse]] — после доставки cancel получит push-ack «принято»
  через тот же канал; текущий polling 1 с покрывает MVP.
- [[active-jobs-store]] — `ActiveJobsStore` уже владеет per-meeting
  poller'ами, добавление `cancel(meetingId)` тривиально.
