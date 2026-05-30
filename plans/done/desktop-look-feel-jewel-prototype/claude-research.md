# Research Findings — Desktop Look-and-Feel (Jewel) Prototype

> Compiled from two parallel Explore subagents: (A) codebase analysis, (B) web
> best-practices on Jewel ↔ Compose compatibility, integration, decorated
> windows, and desktop-native look. Date: 2026-05-17.

---

## ⚠️ HEADLINE FINDING (changes the plan)

**There is no published JetBrains Jewel release that targets Compose
Multiplatform 1.7.3 / Kotlin 2.0.21.**

- Jewel's standalone repo (`github.com/JetBrains/jewel`) is **archived/read-only**
  as of April 2025. Active development moved into `intellij-community`
  (`platform/jewel`).
- Jewel **0.35 (2026-03-30)** and **0.36 (2026-04-27)** both require **CMP
  1.10.0** and IJP 2025.3.x / 2026.1.x.
- CMP 1.8.0+ requires **Kotlin 2.1.0+**. This project is pinned to **CMP 1.7.3 /
  Kotlin 2.0.21**.
- Historical standalone Jewel (≈0.22–0.24) is the only era that *might* line up
  with older CMP, but it is undocumented for 1.7.3 and unsupported.

**Implication for the PRD:** Phase 1's "resolve a Jewel version compatible with
CMP 1.7.3 / Kotlin 2.0.21" is very likely to FAIL as written. The decision is
now a fork that must be made *before* implementation, not discovered mid-spike:

1. **Upgrade the toolchain** (CMP 1.7.3 → 1.10.0, Kotlin 2.0.21 → 2.1.x) so
   real Jewel can be used — strongest "proven-native" signal, but the PRD
   explicitly forbids toolchain changes and this risks the FFI/Skiko stack.
2. **Take the documented fallback now**: custom compact non-Material theme on
   CMP 1.7.3 (no Jewel). Preserves the experiment's core question
   ("theming vs stack limit") with a slightly weaker "proven-native" signal.
3. **Hybrid**: attempt an old standalone Jewel pin in a short timebox; fall
   back to the custom theme if it doesn't resolve/run.

This is the central open question for the interview.

---

## PART A — Codebase Analysis

### A1. Composition root, window, navigation

- **`ui-compose/desktopApp/src/desktopMain/kotlin/Main.kt`**
  - Lines ~67–154: `application { Window(...) }` (standard Compose Desktop
    `Window`, **not** a decorated window). `rememberWindowState()` for
    size/pos; `WindowPrefs` persists position; Decompose `LifecycleRegistry`.
  - Lines ~34–51: native lib path resolved from `-Drust.target.dir` system
    property or packaged resources; `System.setProperty(... libraryOverride ...)`.
  - Lines ~83–106: in `main()`, `initCore(AppConfig(...))` is an **FFI call
    under `runBlocking`**, then `RootComponent` built with four Uniffi
    repositories.
  - **Toggle insertion point:** line ~150, just before `AppContent(root)`.
- **`ui-compose/shared/src/commonMain/kotlin/ui/navigation/RootComponent.kt`**
  - Lines 23–53: holds `meetings/recording/settings/diagnostics` repos;
    `_screen: MutableValue<Screen>` (sealed `Screen` at 14–21); ViewModels via
    `instanceKeeper.getOrCreate(...)`; nav methods `onMeetingSelected`,
    `onSettingsRequested`, `onBackToList`.
- **`ui-compose/shared/src/commonMain/kotlin/ui/AppContent.kt`**
  - Lines 26–44: `AppContent(root)` wraps everything in `AppTheme { }` (line
    27, Material 3 hardcoded), then a `Row`: Sidebar (280.dp) | VerticalDivider
    | ContentPane (weight 1). `ContentPane` (48–60) renders per-screen
    composables via `when` over `root.screen.subscribeAsState()`.
  - **Recommended toggle approach:** add `uiVariant: UiVariant` param to
    `AppContent`, branch the theme/screen set on it; source the value from a
    runtime dev control.

### A2. MeetingList screen

- `ui-compose/shared/src/commonMain/kotlin/ui/screens/MeetingListScreen.kt` is
  effectively a **placeholder** (centered Russian instructions). The real list
  is the **Sidebar**.
- **`ui-compose/shared/src/commonMain/kotlin/ui/Sidebar.kt`** (lines 24–93):
  - Header `SidebarHeader`: title "Встречи" (`titleMedium`), `Icons.Default.Add`
    IconButton (FAB-like, primary tint), refresh button.
  - `LazyColumn` (weight 1) over `MeetingListState.Success.meetings`.
  - `MeetingListItem` (141–175): selected bg `primary.copy(alpha=0.15f)` else
    `surfaceVariant`; shows name (`bodyMedium`), `formatDate(createdAt)`
    (`labelSmall`), conditional `StatusChip("Транскрипт")`/`"Протокол"`.
  - Empty state (69–82): centered "Нет встреч".
  - Footer `SidebarFooter` (88–91): Settings & Diagnostics icon buttons.
- **`viewmodel/MeetingListViewModel.kt`** (21–50): `InstanceKeeper.Instance`;
  `init { load() }`; `MeetingListState = Loading | Success(List<Meeting>) |
  Error(message)`; loads via `repository.list()`.
- **Domain `Meeting`** (`domain/Meeting.kt`): `id, name, audioPath,
  hasTranscript, hasProtocol, createdAt: Long`.

### A3. Settings screen (form-density stress point)

`ui-compose/shared/src/commonMain/kotlin/ui/screens/SettingsScreen.kt`
(27–429). `Scaffold` + `SnackbarHost` + scrolling `Column`. Toolbar
`SettingsToolbar` (85–98): TopAppBar, title "Настройки", back arrow.

`SettingsForm` (102–429) sections & controls:

1. **Anthropic API** (152–178): `OutlinedTextField` "API Key", placeholder
   `sk-ant-…`, password visibility toggle (trailing icon), supporting
   text/validation, single-line; error if not `sk-ant-` prefixed.
2. **Recording** (180–208): `SingleChoiceSegmentedButtonRow` "Источник звука"
   = {Микрофон, Система, Оба}; `Switch` "Подавление эха".
3. **Protocol template** (210–242, conditional): `ExposedDropdownMenuBox`,
   read-only field, items "По умолчанию" + template names.
4. **Transcription** (244–324): language `SingleChoiceSegmentedButtonRow`
   {Русский, English, Авто}; "Точность распознавания" `Slider` 1f–5f/3 steps
   with explanatory text; "CPU потоки" `Slider` 0f–16f/15 steps ("Авто" at 0).
5. **Storage paths** (326–397): two `PathField`s (`OutlinedTextField`,
   placeholder `/home/user/…`, error if non-blank and not `/` or `~`); Model
   section — `ExposedDropdownMenuBox` of models (`"name  ·  sizeMb MB"` +
   description) with "Другой путь…" → custom `PathField`; prompts dir field.
6. **Save** (399–427): full-width 48.dp `Button` "Сохранить",
   `enabled = pathsValid && apiKeyValid`, snackbar feedback.

`Settings` domain object: `paths{model,db,meetingsDir,prompts}`,
`anthropicApiKey`, `recording{source,echoCancel}`, `defaultTemplate`,
`transcriber{language,beamSize(1–5),nThreads(0=auto)}`.

### A4. ViewModels, repositories, faking data

- VMs created in `RootComponent` via `instanceKeeper.getOrCreate`, given
  Uniffi repos. `ProtocolGenerateViewModel` is per-meeting (`remember` +
  `DisposableEffect`).
- Repos (`UniffiMeetingRepository`, `UniffiSettingsRepository`, …) wrap
  `core.*` FFI calls. `initCore(...)` requires the native `.dylib` and runs
  before `RootComponent`.
- **Faking:** repositories are interfaces (`MeetingRepository`,
  `SettingsRepository`, …) in `commonMain`. Cleanest path for the prototype is
  **fake repository implementations with static data** wired in a dev
  composition root that does **not** call `initCore`, so the prototype runs
  without the Rust lib.
  - Mock meetings: a couple of `Meeting(...)` with mixed
    `hasTranscript/hasProtocol`.
  - Mock settings: default `Settings` + a couple of templates + a couple of
    `WhisperModel`s (`domain/WhisperModel.kt`).

### A5. Build setup

- `ui-compose/gradle/libs.versions.toml`: `kotlin = "2.0.21"`,
  `compose = "1.7.3"`, `decompose = "3.2.2"`, `jna = "5.15.0"`,
  `coroutines = "1.8.1"`, `markdown-renderer = "0.30.0"`. Plugins pinned to
  Kotlin 2.0.21 / Compose 1.7.3 / compose-compiler 2.0.21.
- `ui-compose/settings.gradle.kts` repos: `gradlePluginPortal`,
  `mavenCentral`, `google`, `maven("https://maven.pkg.jetbrains.space/public/p/compose/dev")`.
  Jewel would need an added repo
  `https://packages.jetbrains.team/maven/p/kpm/public/` (and possibly
  `intellij-dependencies`).
- `desktopApp/build.gradle.kts`: KMP + Compose plugins;
  `compose.desktop.currentOs`, `compose.material3`, `libs.decompose`,
  `kotlinx.coroutines.swing`; `-Drust.target.dir` / `-Dmeeting.prompts.dir`
  jvmArgs; ProGuard disabled; native-lib copy tasks per OS.
- `shared/build.gradle.kts`: commonMain uses `compose.runtime/foundation/
  material3/materialIconsExtended`, decompose, coroutines, markdown-renderer-m3;
  desktopMain adds `compose.desktop.currentOs`, jna, jna.platform.
- Gradle wrapper: **8.10**.

### A6. Theming

- **`ui-compose/shared/src/commonMain/kotlin/ui/theme/AppTheme.kt`** (8–44):
  hand-rolled dark/light `darkColorScheme/lightColorScheme` (primary
  `0xFF4FA3E0` dark / `0xFF1A6FAF` light, surfaces, onSurface…). `AppTheme(
  darkTheme=true) { MaterialTheme(colorScheme=…) { content } }`.
- App-wide reliance on `MaterialTheme.colorScheme.*` and
  `MaterialTheme.typography.*` (titleMedium, bodyMedium, labelSmall…) plus
  Material components (`Scaffold`, `TopAppBar`, `OutlinedTextField`,
  `Slider`, `Switch`, `SegmentedButton`, `ExposedDropdownMenuBox`, `Button`).
  A parallel theme/screen set must replace these per-component, not just swap
  a color scheme.

---

## PART B — Web Best-Practices

### B1. Jewel ↔ CMP 1.7.3 / Kotlin 2.0.21 (PRIMARY RISK) — verdict: NOT VIABLE

- Jewel 0.36 → CMP 1.10.0, IJP 2025.3.4+/2026.1.1+. Jewel 0.35 → CMP 1.10.0.
  (Source: Jewel RELEASE NOTES in `JetBrains/intellij-community`,
  `platform/jewel/RELEASE NOTES.md`.)
- Kotlin 2.0.21 ↔ CMP 1.7.3 is a valid pair; CMP ≥1.8.0 needs Kotlin ≥2.1.0
  (Source: kotlinlang.org compose-compatibility-and-versioning).
- Artifacts: `org.jetbrains.jewel:jewel-int-ui-standalone:<ver>-<ijpBuild>`,
  `…:jewel-int-ui-decorated-window:<ver>-<ijpBuild>`; repo
  `https://packages.jetbrains.team/maven/p/kpm/public/`.
- Version-skew symptoms when forcing mismatched Compose: Skiko/native vs
  Kotlin code mismatch → opaque runtime crashes; compose-compiler/metadata
  conflicts. Workaround = strictly align `androidx.compose` vs
  `org.jetbrains.compose` versions — fragile.
- **Bottom line:** Do not pair Jewel with CMP 1.7.3. Either upgrade to CMP
  1.10.0 + Kotlin 2.1.x for Jewel 0.35+, or do not use Jewel.

### B2. Jewel standalone integration (if toolchain upgraded)

- Artifacts: `jewel-int-ui-standalone` (+ `jewel-int-ui-decorated-window`).
- Wrap UI in `IntUiTheme { … }`; for chrome use `DecoratedWindow(onCloseRequest,
  title) { IntUiTheme { … } }`.
- **Gotcha:** don't nest Material3 `MaterialTheme` inside `IntUiTheme` (inner
  Material values win and break Jewel styling). Keep Jewel and Material in
  **separate composable subtrees** — which fits the per-variant screen-set
  approach (don't try to share one screen tree).
- CompositionLocals: `LocalJewelTextStyle` (~13.sp compact),
  hover state locals; no Swing bridge in standalone.

### B3. Decorated window / title bar

- `DecoratedWindow` + `TitleBar` gives IntelliJ-style custom chrome
  (min/max/close), undecorated OS frame with Compose-drawn controls.
- **macOS limitations:** no exact native traffic-light placement; custom
  buttons rendered; uses standard Compose decoration, not Cocoa.
- **Fallback:** detect macOS → use standard `Window { IntUiTheme { … } }`,
  keep Jewel *content*, drop the decorated chrome. (Matches PRD edge case.)

### B4. Desktop-native look in Compose (informs the custom-theme fallback)

The "mobile feel" is largely **density/spacing/typography**, addressable
without Jewel:

| Aspect | Material 3 default | Desktop-native |
|---|---|---|
| Button height | ~48.dp | ~28–32.dp |
| Inter-element spacing | ~16.dp | ~6–8.dp |
| Body/UI text | ~16.sp | ~13–14.sp |
| Checkbox/radio | 24×24 | 16×16 |
| Hover | none | explicit hover bg/affordance |

Techniques: compact `LocalDensity`/spacing tokens; slim fixed-height
controls; `Modifier.hoverable`/pointer hover backgrounds; system fonts
(SF Pro/Segoe UI/Inter); custom `LocalScrollbarStyle`; right-click context
menus; tighter `contentPadding`/`PaddingValues`. A minimal non-Material
theme = a `colors/spacing/typography` token object + a handful of compact
composables (button, text field, list row, section header) — enough to
re-skin the two screens.

### Key URLs

- `github.com/JetBrains/intellij-community/tree/master/platform/jewel`
  (+ `platform/jewel/RELEASE NOTES.md`)
- `github.com/JetBrains/jewel` (archived standalone)
- `kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html`
- `packages.jetbrains.team/maven/p/kpm/public/`
- `developer.android.com/develop/ui/compose/designsystems/custom`

---

## Consequences for the Plan

1. **Phase 1 is no longer a discovery spike** — the compatibility answer is
   already known (Jewel ≠ CMP 1.7.3). Phase 1 becomes a *decision* + a thin
   proof of whichever path is chosen.
2. **The PRD's hard constraint ("must build with CMP 1.7.3 / Kotlin 2.0.21")
   conflicts with "use real Jewel".** One must give. → interview question.
3. The **custom compact non-Material theme** path is fully viable on the
   current toolchain and is the lowest-risk way to still answer the core
   question. The PRD already designates it the fallback; research suggests it
   may need to be the *primary* path.
4. Faking data via the existing repository interfaces lets the prototype run
   **without `initCore`/the Rust dylib**, removing the FFI/JDK env risk for the
   visual test.
5. Per-variant **separate screen sets** (not one parameterized tree) is the
   right structure regardless of Jewel vs custom — confirmed by both the
   codebase shape and Jewel's "don't nest themes" rule.
