# Qt Redesign — интеграция дизайна «Meety»

Перенос дизайна из Claude Design (`design/meety/`) на текущий Qt 6 / QML UI.
**Полный редизайн визуального слоя; логика (навигация, MeetingStore, ApiClient,
JobPoller, бэкенд, API) не меняется.**

## Решения (зафиксированы с пользователем)

| Вопрос | Решение |
|---|---|
| Отображение протокола | **Editorial** (длинный серифный текст) |
| Главный элемент записи | Пока без тяжёлых вариантов (orb/wave/live отложены); типографический глиф + таймер + лёгкая анимация (пульс/мини-волна без реальных аудио-данных) |
| Система тем | **Только light + sienna**; `Theme.qml` спроектировать так, чтобы dark/палитры/плотность добавлялись позже без переписывания экранов |
| Рамка окна | **Нативная** рамка ОС; mock-титлбар со «светофором» не переносим |
| Доп-экраны | **Welcome / first-run** делаем; **Command palette (⌘K)** и кастомный титлбар — отложены |

## Технические нюансы (специфика этого дизайна)

1. **`oklch` → sRGB.** Все токены в `design/meety/project/styles.css` заданы в
   `oklch()`, который QML не понимает. Палитра light+sienna уже сконвертирована
   в hex (см. таблицу ниже) — это вшивается в `Theme.qml`.
2. **Шрифты.** Дизайн опирается на Geist (UI), Newsreader (serif — основа
   «издательского» вида), JetBrains Mono (mono). Нужно скачать TTF, положить в
   ресурсы Qt и зарегистрировать (FontLoader / qt_add_resources). Без Newsreader
   вид разваливается.
3. **Анимации.** CSS `@keyframes` (pulse, leak-in, indeterminate, caret) →
   QML `Behavior`/`NumberAnimation`/`states`. Семантика та же, синтаксис другой.

## Палитра light+sienna (hex, готова к вставке в Theme.qml)

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

Радиусы: r-sm 6, r-md 8, r-lg 12, r-xl 16. Density(regular): row-py 10, gap 16.

## Маппинг компонентов

| Дизайн (React) | Цель (QML) |
|---|---|
| `styles.css` токены | `qml/theme/Theme.qml` (singleton) |
| `Sidebar.jsx` | сайдбар в `AppShell.qml` |
| `MeetingDetail.jsx` | `screens/MeetingDetailScreen.qml` |
| `NewRecording.jsx` | `screens/NewRecordingScreen.qml` |
| `GenerateProtocol.jsx` | `screens/GenerateProtocolScreen.qml` + `components/PipelineProgress.qml` |
| `Settings.jsx` | `screens/SettingsScreen.qml` + панели |
| `Welcome.jsx` | новый `screens/WelcomeScreen.qml` |
| `Icons.jsx` | SVG-иконки в ресурсах |
| `CommandPalette.jsx` | отложено |

## Фазы

### Фаза 1 — Дизайн-токены + шрифты (фундамент)
- `qml/theme/Theme.qml` (singleton): цвета (hex выше), типографика (3 семейства +
  размеры/веса), spacing, радиусы, длительности анимаций.
- Скачать и встроить шрифты Geist / Newsreader / JetBrains Mono; зарегистрировать.
- Зарегистрировать singleton в CMake/qmldir.
- **Чекпоинт:** показать тему + образец типографики до правки экранов.

### Фаза 2 — Слой компонентов
Перевести переиспользуемые элементы в `qml/components/`, стилизованные через Theme:
`MeetyButton` (default/primary/accent/ghost/icon/lg), `Field`/`TextField`,
`Segmented`, `Switch`, `Tag`/`Chip`, `Card`, `MenuPopover`, `SectionLabel`.

### Фаза 3 — Экраны (по одному, логику не трогаем)
1. `AppShell` сайдбар (header, search-вид, список встреч с состояниями,
   footer/sync-indicator).
2. `MeetingDetailScreen` — content-bar + протокол **Editorial** (h1/h2/h3/p/ul/
   table, серифная типографика).
3. `NewRecordingScreen` — idle (dropzone/import) + recording (глиф + таймер +
   лёгкая анимация).
4. `GenerateProtocolScreen` / `PipelineProgress` — steps, progressbar
   (determinate + indeterminate), статус-пилюля.
5. `SettingsScreen` + панели — settings-nav + rows + template-grid/prompt-viewer.
6. `WelcomeScreen` (first-run / empty).

### Фаза 4 — Полировка
Hover/active-состояния, анимации (Behavior), фокус-кольца, прокрутка, проверка
длинного контента/локали, светлая тема на всех экранах.

## Вне объёма
Dark/slate/moss/плотности (заложить, не реализовывать), command palette, кастомный
безрамочный титлбар, live-транскрипт и реальная waveform из аудио-уровней,
изменения бэкенда/API.

## Риски
- Шрифты увеличивают размер бандла и попадают под подпись/нотаризацию (учесть в packaging).
- Точность серифной вёрстки протокола (Editorial) — Text с richtext/Markdown vs.
  ручная вёрстка; решить на Фазе 3 п.2.
- `oklch`-конверсия даёт близкий, но не идентичный цвет; светлая тема выверена,
  dark пересчитать при добавлении.
