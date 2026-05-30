# Worker Concurrency Pool

## Problem

Сидекар запускает один [`Worker`](../../../rust/crates/adapters/src/worker.rs),
который сериализованно дренирует очередь jobs: даже на 8-ядерной машине
genereting протокол (HTTP к LLM, ~15-30 c IO-wait) блокирует следующий
`claim_pending`, а bulk-перегенерация 10 протоколов после смены шаблона
выполняется ровно последовательно. При этом `JobRepo::claim_pending` уже
атомарен ([db/job_repo.rs:107-141](../../../rust/crates/adapters/src/db/job_repo.rs#L107-L141)),
а `LazyWhisperTranscriber.transcribe_inner` уже concurrency-safe
([whisper.rs:457-502](../../../rust/crates/adapters/src/whisper.rs#L457-L502))
— `active_count` корректно обрабатывает множественные одновременные вызовы.
Сериализация сидит исключительно в форме воркера: один `loop { claim → execute }`
для обоих kind'ов jobs.

## Goal

Развести два класса ресурсов на два отдельных воркера:

- **TranscribeWorker** — CPU-bound, лимит `cpu_pool = max(1, num_cpus / threads_per_job)`.
- **ProtocolWorker** — IO-bound (HTTP), лимит `io_pool = 4` (по умолчанию,
  защита от rate-limit'ов LLM-провайдера).

Воркеры исполняются независимо, claim'ят только свой `kind`, делят общий
`LiveProgress` и общий graceful-shutdown.

## Expected Outcome

| Сценарий | Сегодня | После | Δ |
|---|---|---|---|
| 1 встреча | 60 c | 60 c | 0% |
| 2 встречи back-to-back | 120 c | 105 c | −12% |
| Bulk regen 10 протоколов | 150 c | 40 c | **−73%** |
| Recovery после рестарта (3 jobs) | 180 c | 135 c | −25% |

Дефолтный пользователь получает overlap T(Mₙ₊₁) **\|\|** P(Mₙ) без тюнинга
настроек; пользователь, регенерирующий протоколы после смены шаблона,
получает 3–4× ускорение. Параллельный whisper остаётся opt-in (требует
явного понижения `n_threads` в Settings — расширяет `cpu_pool > 1`).

## Scope

**В этой задаче:**

- Расширение `JobRepo::claim_pending` до `claim_pending_kind(filter, now_ts)`
  с фильтром по списку `JobKind`'ов. Старая `claim_pending` остаётся как
  тонкий wrapper (без фильтра) для тестов и регрессии.
- Рефакторинг `Worker`: единый struct с полями `kind_filter`, `permits`,
  два инстанса в `Container.spawn_worker` (`TranscribeWorker` и
  `ProtocolWorker` — терминология, не отдельные типы).
- Per-job tasks за `Arc<Semaphore>` через `JoinSet`.
- Graceful shutdown: два worker'а дренируются под общим 1.2 c-бюджетом
  сидекара (`tokio::join!` под `timeout`).
- Логирование `pool_size` (cpu и io) на старте; `aborted_inflight=N`
  при истечении бюджета.

**Вне этой задачи:**

- Отмена jobs ([`backlog/job-cancellation.md`](../../../backlog/job-cancellation.md)).
- SSE для live progress ([`backlog/job-progress-sse.md`](../../../backlog/job-progress-sse.md)).
- Per-provider rate-limit (Anthropic vs OpenAI) — общий `io_pool = 4` достаточен.
- GPU-транскрибер.
- Динамический ресайз пулов при hot-swap `n_threads` — restart-required, как
  `db_path`/`recordings_dir` (см. [RULES.md](../../../RULES.md)).
- Изменение дефолтного `n_threads`: остаётся `0` (auto = `num_cpus`),
  параллельный whisper opt-in.

## Verified prerequisites

- **`LazyWhisperTranscriber` concurrency-safe.** `transcribe_inner` атомарно
  инкрементирует `active_count`, забирает `unload_handle.abort()`, держит
  `Arc<dyn WhisperRunner>` через короткий `Mutex` lock. `WhisperContext`
  shared (read-only после load), `WhisperState` создаётся per-call
  ([whisper.rs:457-502](../../../rust/crates/adapters/src/whisper.rs#L457-L502)).
  Concurrent transcribes не требуют изменений в `whisper.rs`.
- **`LiveProgress` (`Arc<DashMap<String, JobProgress>>`)** уже concurrent.
  Двойная декларация в `meeting-api::router` и `meeting-adapters::worker`
  остаётся риском дрейфа типов; [`live-progress-unification`](../../../backlog/live-progress-unification.md)
  — рекомендуемый, но не блокирующий пререкизит.
- **SQLite write-tx через `BEGIN IMMEDIATE`.** Параллельные `claim_pending`
  сериализуются драйвером, но в новой архитектуре одновременных claim'ов
  максимум **два** (по одному на каждый воркер), что для SQLite на каждые
  ~2 c — не нагрузка.

## Decisions

1. **Два воркера, два пула.** TranscribeWorker (`cpu_pool`) клеймит только
   `Transcribe`/`Reprocess…`; ProtocolWorker (`io_pool`) клеймит только
   `RegenerateProtocol`. Деление по physical resource (CPU vs network).
   - *Почему не один воркер с общим семафором:* при дефолтном `n_threads = 0`
     `cpu_pool = 1`, и protocol-job (IO-bound) занимает единственный permit,
     блокируя следующий transcribe. Default-пользователь не получает win.
   - *Почему не select! по двум семафорам в одном loop'е:* busy-loop hazard
     (если один kind пуст, permit того типа постоянно свободен, claim
     возвращает `None`, нужны per-arm sleep'ы). С двумя воркерами каждый
     sleep'ает независимо.
   - *Статус:* твёрдо.

2. **`cpu_pool = max(1, num_cpus::get_physical() / max(1, threads_per_job))`,
   `io_pool = 4`.** `threads_per_job` берётся из
   `PersistedTranscriberPrefs.n_threads` ([settings_store.rs:40-48](../../../rust/crates/adapters/src/settings_store.rs#L40-L48));
   `n_threads = 0` разворачивается в `num_cpus::get_physical()` (то же
   правило, что в [whisper.rs:140-144](../../../rust/crates/adapters/src/whisper.rs#L140-L144)).
   `io_pool = 4` — компромисс между throughput bulk-regen и rate-limit'ами
   типичного LLM-провайдера (Anthropic free-tier: 50 req/min; 4 одновременных
   ≈ комфортный bound).
   - *Статус:* `io_pool = 4` — рекомендация; пересматривается, если в проде
     увидим rate-limit ошибки.

3. **Permit-before-claim в каждом воркере.** Семафор-permit берётся **до**
   `claim_pending_kind`. Иначе jobs зависают в `running`, ожидая permit,
   и блокируют reconciliation.
   - *Статус:* твёрдо.

4. **Прерывистый shutdown.** `tokio::select!` между `shutdown` и
   `Semaphore::acquire_owned`/`claim_pending_kind` — иначе idle-воркер
   (нет jobs, ждёт claim_pending sleep) выходит из цикла только после
   следующего тика.
   - *Статус:* твёрдо.

5. **JoinSet внутри `Worker::run`.** На shutdown цикл клейма выходит,
   дальше `join_next()` до общего тайм-аута сидекара. Сидекаровский
   1.2 c-бюджет остаётся; **расширяется логированием** `aborted_inflight=N`,
   когда не успели. Abort'нутые jobs остаются `running` в DB и
   восстанавливаются `recover_running_jobs` на следующем старте.
   - *Статус:* твёрдо.

6. **`recover_running_jobs` зовётся **один раз** на старте container'а до
   spawn-loop'ов** (а не в каждом воркере). Сбрасывает все `running` →
   `pending` без знания о kind.
   - *Статус:* твёрдо.

7. **`Worker` остаётся одним struct'ом с полем `kind_filter: &'static [JobKind]`.**
   Не вводим отдельные типы `TranscribeWorker`/`ProtocolWorker` — это
   термины для людей. Реализация: один struct, два инстанса.
   - *Почему:* избегает дупликации кода; вся логика execute/handle_failure
     одинаковая, отличается только фильтр claim'а и pool size.
   - *Статус:* твёрдо.

## Deliverables

### Backend

**1. `rust/crates/core/src/ports/job_repo.rs`** — расширение порта:

```rust
async fn claim_pending_kind(
    &self,
    kinds: &[JobKind],
    now_ts: i64,
) -> Result<Option<Job>, CoreError>;
```

Старая `claim_pending(now_ts)` оставлена как default-метод (или удалена с
обновлением call-site'ов — в проде её зовёт только `Worker::run`, в тестах
~5 мест). **Решение:** удалить `claim_pending`, обновить тесты; одна точка
правды.

**2. `rust/crates/adapters/src/db/job_repo.rs`** — реализация:

```sql
SELECT … FROM jobs
WHERE status='pending' AND retry_after <= ?1
  AND kind IN (rarray(?2))
ORDER BY created_at LIMIT 1
```

Без `rarray` (rusqlite-фича) проще: построить `?, ?, ?` под количество
kind'ов и пробросить параметры. У нас `kinds.len() ∈ {2, 1}` — статично
для двух воркеров; динамический IN не нужен.

**3. `rust/crates/core/src/fakes.rs`** — обновить `FakeJobRepo`:

```rust
async fn claim_pending_kind(&self, kinds: &[JobKind], now_ts: i64)
    -> Result<Option<Job>, CoreError>
{
    // Find first pending whose kind is in `kinds` and retry_after <= now_ts.
}
```

**4. `rust/crates/adapters/src/worker.rs`** — основная переделка:

- Поля `Worker`:
  ```rust
  pub struct Worker {
      job_repo: Arc<dyn JobRepo>,
      meeting_repo: Arc<dyn MeetingRepo>,
      transcriber: Arc<dyn Transcriber>,
      file_store: Arc<dyn MeetingFileStore>,
      llm: Arc<dyn LlmProvider>,
      templates: Arc<dyn TemplateLoader>,
      progress: LiveProgress,
      kind_filter: &'static [JobKind],   // НОВОЕ
      permits: Arc<Semaphore>,           // НОВОЕ
      pool_size: usize,                  // НОВОЕ — для логов
      name: &'static str,                // НОВОЕ — "transcribe" / "protocol"
  }
  ```
- `Worker::new` принимает `kind_filter`, `permits`, `name`.
- `run(self: Arc<Self>, mut shutdown: oneshot::Receiver<()>)`:
  ```text
  // recover_running_jobs() вынесена в container до spawn loop'ов
  let mut inflight = JoinSet::new();
  loop {
      let permit = tokio::select! {
          biased;
          _ = &mut shutdown => break,
          p = self.permits.clone().acquire_owned() => p.expect("not closed"),
      };
      let job = tokio::select! {
          biased;
          _ = &mut shutdown => { drop(permit); break }
          r = self.job_repo.claim_pending_kind(self.kind_filter, now_unix()) => r,
      };
      match job {
          Ok(Some(job)) => {
              let me = Arc::clone(&self);
              inflight.spawn(async move {
                  me.execute(job).await;
                  drop(permit);
              });
          }
          Ok(None)  => { drop(permit); sleep(POLL_INTERVAL).await; }
          Err(e)    => { drop(permit); error!(...); sleep(POLL_INTERVAL).await; }
      }
  }
  // drain
  let mut aborted = 0;
  while inflight.join_next().await.is_some() {}  // sidecar таймаутит снаружи; abort_handle прерывает run, JoinSet::drop отменит остатки
  info!(worker = self.name, "claim loop stopped");
  ```
- `execute`/`run_transcribe`/`run_regenerate_protocol`/`handle_failure`
  — без изменений по сигнатурам (всё на `&self`).
- Добавить `pub fn pool_size(&self) -> usize` и `pub fn in_flight(&self) -> usize`
  (через `pool_size - permits.available_permits()`).

**5. `rust/crates/app/src/container.rs`** — `spawn_worker` → `spawn_workers`:

- В `Container` добавить `pub settings_store: Arc<JsonSettingsStore>` (копия,
  поскольку `settings_handles` забирается в `meeting-server.rs` до `spawn_workers`).
- Подпись:
  ```rust
  pub fn spawn_workers(&self) -> (
      tokio::task::JoinHandle<()>,                   // transcribe
      tokio::sync::oneshot::Sender<()>,              // transcribe shutdown
      tokio::task::JoinHandle<()>,                   // protocol
      tokio::sync::oneshot::Sender<()>,              // protocol shutdown
  )
  ```
- Логика:
  1. `recover_running_jobs(now_unix())` — один раз.
  2. Вычислить `cpu_pool = max(1, num_cpus::get_physical() / max(1, threads_per_job))`,
     `io_pool = 4`.
  3. Логирование `info!(cpu_pool, io_pool, threads_per_job, "worker pools sized")`.
  4. Создать два `Worker`'а с разными `kind_filter` и `permits`, обернуть в `Arc`,
     `tokio::spawn(worker.run(rx))`.

**6. `rust/crates/app/src/bin/meeting-server.rs`** — drain:

```rust
let (t_join, t_shutdown, p_join, p_shutdown) = container.spawn_workers();
// …
// after axum returns:
let _ = t_shutdown.send(());
let _ = p_shutdown.send(());
let drain = tokio::time::timeout(Duration::from_millis(1200),
    async { tokio::join!(t_join, p_join) });
if drain.await.is_err() {
    tracing::warn!("worker pools did not stop within 1.2s — aborting");
    t_abort.abort();
    p_abort.abort();
}
```

### Тесты

**1. `rust/crates/adapters/src/worker.rs`**:

- `transcribe_worker_runs_two_transcribes_concurrently_when_cpu_pool_is_2`:
  cpu_pool=2, два T-job'а, `FakeTranscriber` с `Barrier(2)` — оба входят
  до того, как любой выйдет. Pass: оба `Done` в пределах timeout.
- `transcribe_and_protocol_run_in_parallel`:
  cpu_pool=1, io_pool=2, очередь: T(M1) уже `running` (висит на Notify),
  P(M2) `pending`. ProtocolWorker должен клеймить P(M2) сразу. Pass:
  P(M2) `running` пока T(M1) висит.
- `protocol_worker_drains_bulk_regen`:
  io_pool=4, 10 P-jobs. После 4 параллельных стартов пятый ждёт. Pass:
  в моменте `inflight = 4`.
- `shutdown_blocks_new_claims`:
  T(M1) висит, P(M2) `pending`. Shutdown → ProtocolWorker не должен
  клеймить P(M2) после shutdown сигнала. Pass: P(M2) остаётся `pending`.
- `shutdown_drains_inflight_then_exits`:
  T(M1) `running` (висит на Notify), shutdown, разблокировка — `run`
  возвращается. Pass: T(M1) → `Done`, `run` finished.
- Существующие тесты (`transcription_with_then_protocol_enqueues_protocol_job`,
  `plain_transcription_does_not_chain_protocol`) — продолжают звать
  `execute(&self, Job)`, не сломаются.

**2. `rust/crates/adapters/src/db/job_repo.rs`**:

- `claim_pending_kind_filters_by_kind`: enqueue T + P, claim с `[Transcribe]`
  возвращает T; claim с `[RegenerateProtocol]` возвращает P.
- `claim_pending_kind_respects_retry_after`: то же что
  `claim_pending_respects_retry_after`, но с фильтром.
- `claim_pending_kind_orders_by_created_at`.

**3. `rust/crates/app/tests/sidecar_contract.rs`** (если эти контрактные
тесты уже покрывают SIGTERM, добавить кейс):

- `sigterm_during_two_inflight_jobs_exits_within_budget`: запустить T + P
  одновременно, SIGTERM, проверить что процесс вышел в пределах 1.5 c.

## Functional Requirements

1. **Default параллелизм:** transcribe(M2) и protocol(M1) идут параллельно
   при back-to-back встречах, наблюдаемо через два live-progress entries.
2. **Bulk-regen:** 10 одновременных протокол-jobs выполняются в 4 потока
   (на дефолтных настройках).
3. **CPU-параллелизм opt-in:** при `n_threads = num_cpus / 2` две
   транскрипции идут параллельно.
4. **Single-CPU degrade:** `cpu_pool = 1`, `io_pool = 4` на 1-ядерной
   машине — protocol всё ещё параллельный, transcribe сериализован.
5. **Атомарность claim:** ни один job не клеймится двумя воркерами;
   `kind_filter` гарантирует disjoint claim'ы.
6. **Graceful shutdown:** оба воркера останавливают claim в пределах 1.2 c,
   in-flight tasks дренируются best-effort, не успевшие — abort'ятся
   и recovery'ятся на следующем старте; в логе `aborted_inflight=N`.
7. **Idle behaviour:** оба воркера не жрут CPU при пустой очереди — permit
   дропается до `sleep(POLL_INTERVAL)`.

## Edge Cases

- **`num_cpus::get_physical() == 1`** → `cpu_pool = 1`, `io_pool = 4`.
  Поведение transcribe = сегодня; protocol параллельный.
- **`n_threads = 0` (auto)** → `threads_per_job = num_cpus` → `cpu_pool = 1`.
  Стандартный случай.
- **`n_threads > num_cpus`** → `cpu_pool = 1`. Не выходим за CPU.
- **Только T-jobs в очереди** → io_pool idle (4 permits свободны),
  ProtocolWorker sleeps на пустых claim'ах. Никакой busy-loop, нет
  лишних SQLite-запросов чаще `POLL_INTERVAL`.
- **Только P-jobs в очереди** → cpu_pool idle; TranscribeWorker sleeps.
- **Recovery `running` → `pending` после рестарта:** делается **один раз**
  в `Container.spawn_workers` до spawn'а воркеров — они не пытаются делать
  свой reconcile.
- **Hot-swap `n_threads`:** новый thread-count применяется к **новым**
  транскрипциям через `LazyWhisperTranscriber.set_prefs`, но `cpu_pool` не
  пересчитывается. Restart-required для изменения размера пула.
- **`POLL_INTERVAL` sleep с зажатым permit'ом** — запрещено. На `Ok(None)`
  и `Err(...)` permit дропается **до** sleep.
- **`shutdown` во время `permits.acquire_owned()`** — `tokio::select!`
  обеспечивает прерывание (Decision #4).

## Acceptance Criteria

- [ ] `JobRepo::claim_pending_kind(kinds, now)` реализован в `SqliteJobRepo`
      и `FakeJobRepo`; `claim_pending` удалён, call-site'ы обновлены.
- [ ] `Worker` принимает `kind_filter`, `permits`, `name`; `run` принимает
      `self: Arc<Self>`, выполняет permit-before-claim, спавнит per-job
      tasks в `JoinSet`.
- [ ] `Container.spawn_workers` создаёт два инстанса (T и P), запускает
      `recover_running_jobs` один раз до них, логирует `cpu_pool`/`io_pool`.
- [ ] `meeting-server.rs` дренирует оба воркера через `tokio::join!` под
      1.2 c-бюджетом.
- [ ] Юнит-тест `transcribe_worker_runs_two_transcribes_concurrently_when_cpu_pool_is_2` зелёный.
- [ ] Юнит-тест `transcribe_and_protocol_run_in_parallel` зелёный.
- [ ] Юнит-тест `protocol_worker_drains_bulk_regen` зелёный.
- [ ] Юнит-тест `shutdown_drains_inflight_then_exits` зелёный.
- [ ] Юнит-тест `shutdown_blocks_new_claims` зелёный.
- [ ] Существующие тесты воркера (`transcription_with_then_protocol_*`,
      `plain_transcription_*`) не сломаны.
- [ ] Локальная проверка на 8-ядерной машине: bulk regen 10 протоколов
      идёт в 4 параллельных HTTP-вызова; back-to-back T+P видны как два
      pipeline-progress'а в QML sidebar'е.

## Implementation Phases

1. **Repo расширение.** `claim_pending_kind` в порту, SQLite, fake; удалить
   `claim_pending`, обновить call-site'ы и тесты. Зелёный регресс.
2. **Worker рефакторинг.** `Arc<Semaphore>` + `kind_filter` + `JoinSet` в
   `run`. `Container.spawn_workers` (вычисление пулов, два инстанса,
   recovery один раз). Минимальная драйв-проверка: всё компилится,
   существующие тесты пасс.
3. **Тесты.** Concurrent T, T \|\| P, bulk-regen, shutdown drain/block.
4. **Sidecar drain.** `tokio::join!` под `timeout(1.2s)` + log на abort.
5. **Closing step.** Сделать **closing commit**, который выполняет
   `git mv plans/active/worker-concurrency-pool plans/done/worker-concurrency-pool`.

## Depends On / Related

- [[live-progress-unification]] — рекомендуемый пререкизит (single source
  of truth для `LiveProgress`); не блокирующий.
- [[job-cancellation]] — построится поверх per-job task handle; раздельные
  пулы упрощают семантику cancel.
- [[job-progress-sse]] — выиграет от стабильного in-flight set.
- [[whisper-gpu-acceleration]] — TranscribeWorker станет точкой расширения
  для GPU-runner'а.
