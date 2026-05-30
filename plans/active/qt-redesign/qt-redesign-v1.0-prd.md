# Qt Redesign (Meety) — Product Requirements Document (PRD)

## Requirements Description

### Background
- **Business Problem**: текущий Qt/QML UI использует голый Fusion-стиль без
  дизайн-системы (см. guardrail "no restyling" в `AppShell.qml`). Это
  расходится с целью проекта — полированное, готовое к релизу приложение.
  Готов дизайн «Meety» (Claude Design, `design/meety/`) — цельная издательская
  система (тёплая бумага + один акцент).
- **Target Users**: конечные пользователи десктоп-приложения Meeting Assistant
  (запись/транскрипция/протоколы встреч).
- **Value Proposition**: профессиональный, узнаваемый внешний вид; единая
  дизайн-система (`Theme.qml`), на которой дешевле развивать UI дальше.

### Feature Overview
- **Core Features**: полный редизайн визуального слоя QML под дизайн «Meety» —
  дизайн-токены, библиотека компонентов, переоформление всех экранов + новый
  Welcome/first-run.
- **Feature Boundaries**:
  - **В объёме**: `Theme.qml`, встраивание шрифтов, слой компонентов, редизайн
    сайдбара и 5 экранов, Welcome (first-run), light+sienna тема.
  - **Вне объёма**: dark/slate/moss темы и плотности (заложить в Theme, не
    реализовывать), command palette (⌘K), кастомный безрамочный титлбар,
    live-транскрипт и реальная waveform из аудио-уровней, **любые изменения
    бэкенда/API/логики** (MeetingStore, ApiClient, JobPoller, Rust core).
- **User Scenarios**: первый запуск → Welcome; повседневно — список встреч в
  сайдбаре, просмотр протокола (Editorial), новая запись, генерация протокола,
  настройки.

### Detailed Requirements
- **Input/Output**: визуальный слой потребляет те же модели/сигналы, что и
  сейчас (`store.meetings`, `store.protocolFor(id)`, `Request`, `PipelineProgress`).
  Протокол приходит как **Markdown** (подтверждено: `llm_provider.rs` «generate
  protocol markdown»).
- **User Interaction**: навигация и поведение экранов сохраняются 1:1; меняется
  только оформление. Поведенческие контракты экранов не редактируются.
- **Data Requirements**: новых данных не вводится. Welcome-триггер требует
  персистентного флага «первый запуск» (см. ниже).
- **Edge Cases**: длинные названия встреч (elide), пустой/loading/error/empty
  состояния списка, отсутствие протокола (CTA), длинный протокол (скролл),
  локаль (русские строки, форматирование дат), отсутствие шрифта (fallback).

## Design Decisions

### Технический подход
- **Architecture Choice**: завести `qml/theme/Theme.qml` (`pragma Singleton`)
  как единый источник токенов — аналог CSS-переменных `:root`. Все экраны
  ссылаются на `Theme.*` вместо `palette.*`. Логика и композиционный корень не
  трогаются.
- **Key Components**:
  - `Theme.qml` — цвета (oklch→hex, см. ниже), типографика, spacing, радиусы,
    длительности анимаций; структурирован так, чтобы dark/палитры/плотность
    добавлялись позже точечно.
  - Шрифты: Geist (UI), Newsreader (serif), JetBrains Mono (mono) — **скачиваю
    сам** (открытые лицензии), кладу в репо, регистрирую через ресурсы Qt /
    FontLoader.
  - `qml/components/`: `MeetyButton` (default/primary/accent/ghost/icon/lg),
    `Field`, `Segmented`, `Switch`, `Tag`/`Chip`, `Card`, `MenuPopover`,
    `SectionLabel`.
- **Interface Design**: внешних интерфейсов не добавляется. Welcome-флаг хранится
  в существующем `JsonSettingsStore` (`settings.json`) либо QSettings на стороне
  Qt — выбрать на Фазе 3 (предпочтительно Qt-сторона, чтобы не трогать core).

### Палитра light+sienna (oklch → sRGB hex)
```
paper      #FBF9F5    ink        #1F1A15
paperSub   #F5F1EC    ink2       #47413C
paper3     #EDE9E2    ink3       #77706A
paper4     #E3DDD5    ink4       #A9A49E
rule       #DBD7D0    rule2      #C9C3BC
accent     #C45E3D    accent2    #B84221
accentTint #FFE3D8    accentInk  #FFFFFF
rec        #DF202E    ok         #2E9052    warn  #CD9130
```
Радиусы: 6/8/12/16. Density(regular): row-py 10, gap 16.

### Constraints
- **Performance**: без регрессий — анимации лёгкие (Behavior/NumberAnimation),
  без постоянных таймеров в idle.
- **Compatibility**: macOS/Linux/Windows; нативная рамка окна (mock-титлбар не
  переносим).
- **Security**: не затрагивается (визуальный слой).
- **Scalability**: Theme спроектирован под будущие dark/палитры/плотности.

### Risk Assessment
- **Technical**: Qt `MarkdownText` ограниченно стилизуется → точный Editorial-вид
  h1/h2/таблиц может потребовать парсинга Markdown в кастомную вёрстку. Решение
  на Фазе 3 п.2 (сначала пробуем стилизованный MarkdownText).
- **Dependency**: встраивание шрифтов увеличивает бандл и попадает под
  подпись/нотаризацию — учесть в `packaging/`.
- **Schedule**: поэкранное согласование удлиняет цикл, но снижает риск
  переделок.
- **Color**: oklch→hex даёт близкий, но не идентичный цвет; light выверен, dark
  пересчитать при добавлении.

## Acceptance Criteria

### Functional Acceptance
- [ ] `Theme.qml` (singleton) содержит палитру light+sienna, типографику,
      spacing, радиусы; зарегистрирован и доступен из QML.
- [ ] Шрифты Geist/Newsreader/JetBrains Mono встроены и применяются.
- [ ] Слой `qml/components/` покрывает кнопки, поля, segmented, switch, tag/chip,
      card, popover-меню, section-label.
- [ ] Сайдбар (`AppShell`) переоформлен: header, поиск-вид, список с 4
      состояниями (loading/empty/error/success), footer.
- [ ] `MeetingDetailScreen` — content-bar + Editorial-рендер протокола.
- [ ] `NewRecordingScreen` — idle (dropzone/import) + recording (глиф+таймер+
      лёгкая анимация, без реальных аудио-данных).
- [ ] `GenerateProtocolScreen`/`PipelineProgress` — steps + progressbar
      (determinate+indeterminate) + статус-пилюля.
- [ ] `SettingsScreen` + панели — settings-nav, rows, template-grid/prompt-viewer.
- [ ] `WelcomeScreen` показывается по флагу первого запуска, затем — empty.
- [ ] Логика (MeetingStore/ApiClient/JobPoller/бэкенд/API) не изменена.

### Quality Standards
- [ ] Сборка проходит (`./run-qt.sh --skip-rust`), `cargo test` не затронут.
- [ ] Нет регрессий поведения экранов (навигация, reprocess, delete, генерация).
- [ ] Анимации не грузят CPU в покое.
- [ ] Русские строки и форматирование дат сохранены.

### User Acceptance
- [ ] **Поэкранное согласование**: каждый экран показан пользователю и одобрен
      до перехода к следующему.
- [ ] Светлая тема (sienna) консистентна на всех экранах.
- [ ] План/PRD в `.claude/plans/qt-redesign/` актуальны.

## Execution Phases

### Phase 1: Фундамент (токены + шрифты)
**Goal**: дизайн-система готова, прежде чем трогать экраны.
- [ ] Скачать Geist/Newsreader/JetBrains Mono (TTF) в репо.
- [ ] Создать `qml/theme/Theme.qml` (singleton) с токенами light+sienna.
- [ ] Зарегистрировать шрифты и singleton (CMake/qmldir/qt_add_resources).
- [ ] Демо-экран: палитра + типографика.
- **Deliverables**: Theme.qml, встроенные шрифты, демо.
- **Чекпоинт**: согласование темы с пользователем.

### Phase 2: Слой компонентов
**Goal**: переиспользуемые QML-компоненты на Theme.
- [ ] MeetyButton (варианты), Field/TextField, Segmented, Switch.
- [ ] Tag/Chip, Card, MenuPopover, SectionLabel.
- **Deliverables**: `qml/components/*` + мини-витрина.

### Phase 3: Экраны (по одному, с согласованием)
**Goal**: переоформить UI без правки логики.
- [ ] Сайдбар (`AppShell`).
- [ ] `MeetingDetailScreen` + Editorial-протокол (решить: стилизованный
      MarkdownText vs кастомный парсинг).
- [ ] `NewRecordingScreen` (idle + recording).
- [ ] `GenerateProtocolScreen`/`PipelineProgress`.
- [ ] `SettingsScreen` + панели.
- [ ] `WelcomeScreen` + флаг первого запуска.
- **Deliverables**: переоформленные экраны, одобренные поэкранно.

### Phase 4: Полировка
**Goal**: довести до релизного качества.
- [ ] Hover/active/focus-состояния, анимации (Behavior), прокрутка.
- [ ] Проверка длинного контента/локали/edge-состояний.
- [ ] Учесть шрифты в `packaging/` (подпись/нотаризация).
- **Deliverables**: финальный проход, чек-лист acceptance закрыт.

---

**Document Version**: 1.0
**Created**: 2026-05-21
**Clarification Rounds**: 3 (варианты дизайна → решения → acceptance/шрифты/welcome)
**Quality Score**: 95/100
