# Global Active-Jobs Store

**Status:** Backlog

## Context

Во время обработки встречи прогресс job‑а живёт как локальный property у
`MeetingDetailScreen` / `GenerateProtocolScreen` (`activeJobId` / `jobId`). При
навигации на другую вкладку — другая встреча, настройки, диагностика — `AppShell`
делает `stack.pop(null)` ([AppShell.qml:39-87](../qt-app/qml/AppShell.qml#L39-L87)),
экран уничтожается, вместе с ним — `PipelineProgress` и его `JobPoller`
([PipelineProgress.qml:110-128](../qt-app/qml/components/PipelineProgress.qml#L110-L128)),
поллинг `GET /api/v1/jobs/:id` останавливается.

При возврате к той же встрече создаётся новый `MeetingDetailScreen`. Запрос
`GET /api/v1/meetings/:id` снова видит «протокол пустой», срабатывает
[MeetingDetailScreen.qml:44-53](../qt-app/qml/screens/MeetingDetailScreen.qml#L44-L53)
и показывает `MeetingNoProtocolScreen` с текстом «Протокол ещё не сгенерирован»,
хотя на бэкенде job всё ещё идёт.

Оба шага конвейера (транскрипция и протокол) уже проходят через единый
job‑механизм `POST /api/v1/meetings/:id/reprocess → GET /api/v1/jobs/:id`
(см. `GenerateProtocolScreen`, синхронный `/api/v1/transcribe` удалён), поэтому
один стор может покрыть оба одинаково.

## Goal

Прогресс активных jobs должен переживать любую навигацию между экранами в рамках
одного запуска приложения. Один источник правды о том, «что сейчас
обрабатывается».

## Scope

**В этой задаче:**
- QML‑singleton `ActiveJobsStore` (по образцу
  [SettingsStore.qml](../qt-app/qml/SettingsStore.qml), регистрация через
  `QT_QML_SINGLETON_TYPE` в [CMakeLists.txt](../qt-app/CMakeLists.txt#L93-L110)).
- Один долгоживущий `JobPoller` на каждый активный job, владелец — стор, а не
  экран.
- Покрытие обоих видов jobs (`transcribe` и `protocol`) одинаково.
- Перерисовка `PipelineProgress` на `MeetingDetailScreen` /
  `MeetingNoProtocolScreen` из данных стора при навигации туда‑обратно.
- Кратковременное удержание терминальной записи (см. «Жизненный цикл»).
- Маленький индикатор «в работе» в строке встречи в сайдбаре
  ([AppShell.qml:367-479](../qt-app/qml/AppShell.qml#L367-L479)).

**Вне этой задачи (отдельные backlog‑пункты):**
- Засев стора активными jobs после рестарта приложения и счётчик‑бейдж в
  сайдбаре — `resume-in-flight-jobs-on-restart.md`.
- Перевод транскрипции на job‑механизм — `transcription-progress-visibility.md`
  (уже выполнено).

## Data Model

`ActiveJobsStore` хранит карту `meetingId → entry`:

```
entry = {
    jobId:   string,   // активный job для этой встречи
    kind:    string,   // "transcribe" | "protocol"
    status:  string,   // "pending" | "running" | "done" | "failed"
    job:     object,    // последний декодированный JobResponse (progress, error_class, ...)
    terminalAt: number  // 0 пока активен; Date.now() при done/failed
}
```

Инвариант: на одну встречу — не более одной активной записи. Новый enqueue для
встречи заменяет предыдущую запись (и её поллер).

## Store API

- `track(meetingId, jobId, kind)` — регистрирует job, поднимает для него
  `JobPoller`. Вызывается из всех точек enqueue.
- `entryFor(meetingId) → entry | null` — снимок для экрана/сайдбара.
- `isActive(meetingId) → bool` — есть незавершённый job (для индикатора).
- `activeCount() → int` — число незавершённых (пригодится resume‑задаче).
- сигнал/`NOTIFY` об изменении карты, чтобы биндинги на экранах и в сайдбаре
  обновлялись.

`JobPoller` ([JobPoller.h](../qt-app/src/JobPoller.h)) — creatable C++ тип;
N экземпляров создаются стором динамически (например, `Instantiator` с моделью
активных `meetingId`, делегат — `JobPoller`, либо явное создание объектов).
`apiClient` берётся из глобального `api`.

## Functional Requirements

1. Каждый активный job поллится ровно одним `JobPoller`, живущим в сторе, вне
   зависимости от того, какой экран открыт.
2. `MeetingDetailScreen` и `MeetingNoProtocolScreen` при загрузке сначала
   спрашивают стор: если для встречи есть активная запись — рендерят
   `PipelineProgress` с её `jobId`; иначе работают как сейчас.
3. `showNoProtocol` срабатывает только когда `protocol == ""` **и** активной
   записи для встречи в сторе нет.
4. Все точки enqueue вызывают `track(...)`:
   - reprocess из меню детали — `transcribe` / `protocol`
     ([MeetingDetailScreen.qml:36-40](../qt-app/qml/screens/MeetingDetailScreen.qml#L36-L40));
   - двухшаговая цепочка в `GenerateProtocolScreen` (включая авто‑старт после
     остановки записи и перегенерацию) —
     [GenerateProtocolScreen.qml:67-95](../qt-app/qml/screens/GenerateProtocolScreen.qml#L67-L95).
     При переходе `transcribe → protocol` запись встречи обновляется на новый
     `jobId`.
5. Экраны больше не владеют собственным `JobPoller` для уже отслеживаемого
   стором job‑а — `PipelineProgress` подключается к стору, не плодя второй
   поллер на тот же `jobId`.
6. В строке встречи в сайдбаре показывается компактный индикатор «в работе»,
   биндинг — `ActiveJobsStore.isActive(meetingId)`.

## Lifecycle (терминальные записи)

- При `done`/`failed` запись **не** удаляется сразу: проставляется `terminalAt`,
  поллер останавливается, запись остаётся доступной короткое время.
- Это окно нужно, чтобы при возврате на экран сразу после завершения было видно
  итог/ошибку и не мелькало ложное «нет протокола» между finish и refresh
  данных встречи.
- Запись снимается, когда наступит раньше: (а) `store.refresh()` подтвердил
  свежие данные встречи, или (б) истёк короткий TTL после `terminalAt`.
  Конкретное значение TTL — деталь реализации (порядка нескольких секунд).

## Edge Cases

- Навигация прочь и обратно во время `transcribe` и во время `protocol` —
  прогресс с процентом и подстадией сохраняется в обоих случаях.
- Свап `jobId` в цепочке транскрипция → протокол не оставляет «залипшего»
  терминального `done` от предыдущего шага (ср. `PipelineProgress.start()`).
- Ошибка job‑а (`failed`) переживает навигацию: при возврате виден экран ошибки
  с локализованным `error_class` и кнопкой «Открыть настройки», где применимо.
- Удаление встречи во время активного job — запись чистится, висячий поллер не
  остаётся.
- `api` ещё не сконфигурирован — `track` безопасно откладывается / не падает
  (поллер не стартует без `apiClient`).

## Acceptance Criteria

- [ ] `ActiveJobsStore` зарегистрирован как QML‑singleton и доступен из любого
      экрана.
- [ ] Переключение на другую встречу / настройки / диагностику и обратно во
      время транскрипции **или** генерации протокола сохраняет видимый прогресс
      с процентом и подстадией.
- [ ] Не возникает ложного «Протокол ещё не сгенерирован», пока на бэкенде идёт
      работа по этой встрече.
- [ ] После завершения (`done`) при возврате на экран кратко виден итог, затем
      подгружается готовый протокол; при `failed` виден экран ошибки.
- [ ] На активную встречу в любой момент приходится не более одного `JobPoller`.
- [ ] В строке встречи в сайдбаре показан индикатор «в работе», пока для неё
      есть активный job, и исчезает по завершении.
- [ ] Старые экраны не создают параллельный поллер для job‑а, уже отслеживаемого
      стором.

## Implementation Phases

1. **Стор.** Создать `ActiveJobsStore` (singleton + CMake‑регистрация), модель
   данных, `track/entryFor/isActive/activeCount`, динамические `JobPoller`,
   логика терминального TTL.
2. **Интеграция enqueue.** Перевести точки reprocess / двухшаговую цепочку
   `GenerateProtocolScreen` на `track(...)`.
3. **Рендер из стора.** `MeetingDetailScreen` / `MeetingNoProtocolScreen`
   читают стор; гейт `showNoProtocol` учитывает активную запись;
   `PipelineProgress` подключается к стору без дублирующего поллера.
4. **Сайдбар.** Индикатор «в работе» в строке встречи.
5. **Проверка.** Прогнать сценарии edge cases вручную в приложении (навигация
   во время обоих шагов, ошибка, удаление во время job).

## Depends On / Related

- `transcription-progress-visibility.md` — выполнено; благодаря ему стор
  покрывает оба шага одним механизмом.
- `resume-in-flight-jobs-on-restart.md` — строится поверх этого стора (засев
  после рестарта + счётчик‑бейдж в сайдбаре).
