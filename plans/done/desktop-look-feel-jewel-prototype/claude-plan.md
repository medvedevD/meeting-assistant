# Implementation Plan — Desktop Look-and-Feel (Jewel) Prototype

> **Read this first — full context for an engineer with zero prior knowledge.**

## 0. What we are building and why

**Meeting Assistant** is a desktop app: a Rust core (audio capture,
transcription, protocol generation) exposed over UniFFI to a **Kotlin
Multiplatform + Compose Desktop** UI (`/ui-compose`). The UI today uses stock
**Material 3** and the project owner thinks it "feels mobile". That single
complaint is the only surviving argument for throwing the whole stack away and
migrating to Qt.

This task is a **cheap empirical test**, not a feature. We build a
**throwaway, never-merged** prototype that re-renders two existing screens
(MeetingList, Settings) with **JetBrains Jewel** (the desktop component
library that gives IntelliJ-grade native-feeling widgets), put it behind a
**runtime toggle** against the current Material 3 screens, and let the owner
look at both and decide: *"this reads as a desktop app"* — or not. The verdict
closes the Qt-vs-Compose decision. **Nothing here ships.**

### The pivotal finding (why this plan diverges from the PRD)

The original PRD (`prd-v1.0.md`) assumed Jewel might work on the project's
pinned **Compose Multiplatform 1.7.3 / Kotlin 2.0.21** and treated a custom
non-Material theme as the fallback. **Research disproved the assumption:**
there is **no Jewel release for CMP 1.7.3 / Kotlin 2.0.21**. Jewel 0.35+/0.36
require **CMP 1.10.0**, and CMP ≥1.8 requires **Kotlin ≥2.1.0**.

The owner's decision (recorded in `claude-interview.md`): **upgrade the
toolchain on the throwaway branch and use real Jewel** — do *not* fall back to
a custom theme unless Jewel cannot run even after the upgrade. Because the
branch is never merged, the upgrade's blast radius is zero.

### Binding decisions (from the interview — these are not open)

- **D1** Upgrade on the branch: CMP 1.7.3 → **1.10.0**, Kotlin 2.0.21 →
  **2.1.x**, `compose-compiler` to match Kotlin; bump Decompose /
  markdown-renderer / coroutines / Gradle as needed. The PRD's "no toolchain
  change" rule is **void on this branch**.
- **D2** Real Jewel: `jewel-int-ui-standalone` + `jewel-int-ui-decorated-window`
  (0.35+/0.36 line for CMP 1.10) from `https://packages.jetbrains.team/maven/p/kpm/public/`.
- **D3** Fake data, **no FFI**: fake repositories with static data; a dev
  entry point that never calls `initCore()` and never loads the Rust dylib.
  Runs on a bare JDK.
- **D4** Jewel variant uses **`DecoratedWindow` + `TitleBar`**; Material
  variant uses the standard `Window`. macOS standard-window fallback (keeping
  Jewel content) behind a constant.
- **D5** Visible **in-window** toggle; **default = Jewel on launch**; flips
  live, **no process restart**; MeetingList↔Settings navigation works in both.
- **D6** Fidelity = **content & density, not pixels**. Scope = exactly the two
  screens.
- **D7** **Only the comparison must compile/run.** Unrelated screens may be
  stubbed/excluded on the branch if they block the upgrade.
- **D8** Branch **`proto/jewel-look-feel`** off `feat/compose-desktop-rewrite`;
  never merged. Assistant delivers a building branch + exact commands +
  README; **the owner runs the GUI and records the verdict** (the assistant
  cannot launch a desktop UI).

## 1. Codebase orientation (what exists today)

Module: `ui-compose` (Gradle, KMP). Relevant files:

- **`desktopApp/src/desktopMain/kotlin/Main.kt`** — `fun main()` →
  `application { Window(...) }`. Calls `initCore(AppConfig(...))` (blocking
  **FFI**, needs the Rust `.dylib`), builds `RootComponent` with four Uniffi
  repository implementations, renders `AppContent(root)`. Native lib path from
  `-Drust.target.dir`.
- **`shared/src/commonMain/kotlin/ui/navigation/RootComponent.kt`** — holds
  the four repository **interfaces** (`MeetingRepository`,
  `RecordingRepository`, `SettingsRepository`, `DiagnosticsRepository`),
  `_screen: MutableValue<Screen>` (sealed `Screen`), ViewModels via
  Decompose `instanceKeeper.getOrCreate`, nav methods (`onMeetingSelected`,
  `onSettingsRequested`, `onBackToList`, …). **Repository types are
  interfaces in `commonMain`** → faking is trivial and touches no shared code.
- **`shared/src/commonMain/kotlin/ui/AppContent.kt`** — `AppContent(root)`
  wraps content in `AppTheme { }` (Material 3, ~line 27), then a `Row`:
  `Sidebar` (280.dp) | `VerticalDivider` | `ContentPane` (per-`Screen`
  `when`). **This is the Material variant we compare against — reuse it as-is.**
- **`shared/src/commonMain/kotlin/ui/Sidebar.kt`** (24–93) — the *real*
  MeetingList: header "Встречи" + add/refresh `IconButton`s; `LazyColumn` of
  `MeetingListItem` (name `bodyMedium`; `formatDate(createdAt)` `labelSmall`;
  conditional "Транскрипт"/"Протокол" chips; selected row highlight); empty
  state "Нет встреч"; footer Settings/Diagnostics icons. State =
  `MeetingListState { Loading | Success(List<Meeting>) | Error(msg) }`.
  `Meeting { id, name, audioPath, hasTranscript, hasProtocol, createdAt:Long }`.
- **`shared/src/commonMain/kotlin/ui/screens/SettingsScreen.kt`** (27–429) —
  `Scaffold` + `SnackbarHost`; `SettingsToolbar` (TopAppBar, "Настройки",
  back arrow); `SettingsForm` with sections:
  1. **Anthropic API**: `OutlinedTextField` "API Key", `sk-ant-…` placeholder,
     password show/hide trailing icon, validation supporting text.
  2. **Recording**: `SingleChoiceSegmentedButtonRow` "Источник звука"
     {Микрофон, Система, Оба}; `Switch` "Подавление эха".
  3. **Protocol template** (conditional): `ExposedDropdownMenuBox`
     "По умолчанию" + template names.
  4. **Transcription**: language `SingleChoiceSegmentedButtonRow`
     {Русский, English, Авто}; "Точность распознавания" `Slider` 1–5/3 steps
     + explanatory text; "CPU потоки" `Slider` 0–16/15 steps (0="Авто").
  5. **Storage**: two `PathField` (`OutlinedTextField`, `/home/user/…`
     placeholder, error if non-blank & not `/`/`~`); model
     `ExposedDropdownMenuBox` (`"name  ·  sizeMb MB"` + description) +
     "Другой путь…" → custom `PathField`; prompts dir field.
  6. **Save**: full-width `Button` "Сохранить",
     `enabled = pathsValid && apiKeyValid`, snackbar feedback.
  `Settings { paths{model,db,meetingsDir,prompts}, anthropicApiKey,
  recording{source,echoCancel}, defaultTemplate,
  transcriber{language,beamSize 1–5,nThreads 0=auto} }`.
- **`shared/src/commonMain/kotlin/ui/theme/AppTheme.kt`** — hand-rolled
  dark/light Material color schemes (dark primary `0xFF4FA3E0`); app defaults
  to **dark**. App pervasively uses `MaterialTheme.colorScheme/typography`.
- **Build**: `ui-compose/gradle/libs.versions.toml` (kotlin `2.0.21`, compose
  `1.7.3`, decompose `3.2.2`, markdown-renderer `0.30.0`, coroutines `1.8.1`,
  jna `5.15.0`); `ui-compose/settings.gradle.kts` repos (mavenCentral, google,
  `maven.pkg.jetbrains.space/public/p/compose/dev`);
  `ui-compose/desktopApp/build.gradle.kts` (Compose Desktop `application`
  block, native-lib copy tasks, `-Drust.target.dir` jvmArgs, ProGuard
  disabled); `ui-compose/shared/build.gradle.kts` (commonMain compose +
  decompose + markdown-renderer-m3; desktopMain jna). Gradle wrapper **8.10**.

> Markdown note: `markdown-renderer-m3 0.30.0` is used by the Protocol-detail
> screen. It is **not** one of the two target screens. Under **D7** it is the
> prime candidate to stub if it blocks the CMP 1.10 upgrade.

## 2. Prototype architecture

**Principle:** isolate everything in a `prototype` package under
`desktopApp/src/desktopMain` and **do not edit `shared`** beyond the
sanctioned Gradle/version-catalog bumps and (only if forced by the upgrade)
unrelated-screen stubs. The Material variant for comparison **is the existing
`AppContent(root)`** reused unchanged. The Jewel variant is a parallel screen
set we write.

```
ui-compose/desktopApp/src/desktopMain/kotlin/
  PrototypeMain.kt            # fun main(): the dev entry. NO initCore, NO dylib.
  prototype/
    UiVariant.kt              # enum { Material, Jewel }
    PrototypeRoot.kt          # composition root: toggle bar + window choice
    fakes/
      FakeMeetingRepository.kt    # implements MeetingRepository (commonMain iface)
      FakeSettingsRepository.kt   # implements SettingsRepository
      FakeRecordingRepository.kt  # minimal stub
      FakeDiagnosticsRepository.kt# minimal stub
      SampleData.kt               # static meetings, settings, templates, models
    jewel/
      JewelTheme.kt           # IntUiTheme (dark) wrapper + shared tokens
      JewelAppShell.kt        # Row: Jewel sidebar | divider | content pane
      JewelMeetingListScreen.kt
      JewelSettingsScreen.kt
      JewelComponents.kt      # small shared helpers (section header, etc.)
```

> **API-shape caveat (read before trusting any Kotlin in this section).**
> All Kotlin below is **illustrative pseudocode**. Three things MUST be
> verified against the actual code/library before relying on them:
> 1. **`RootComponent` + repository interface signatures** — read
>    `RootComponent.kt` and each repository interface in `commonMain` and
>    match constructor param order/names, the exact suspend signatures,
>    return types, nullability, and that `meetingListViewModel` /
>    `recordingViewModel` are `public`. Write fakes to the *real* signatures.
> 2. **Decompose lifecycle** — `Main.kt` does specific lifecycle wiring
>    (`LifecycleRegistry`, and on desktop typically `lifecycle.resume()` or
>    `LifecycleController(lifecycle, windowState)`). `RootComponent` builds
>    ViewModels via `instanceKeeper.getOrCreate` and `MeetingListViewModel`
>    runs `init { load() }`; **if the lifecycle is not resumed/bound exactly
>    as in `Main.kt`, screens render blank.** Replicate `Main.kt`'s lifecycle
>    setup verbatim in `PrototypeMain.kt`.
> 3. **Jewel `DecoratedWindow` / `TitleBar`** — these have a specific
>    `DecoratedWindowScope`/slot contract that differs by Jewel version. The
>    `Column { TitleBar(); toggle(); shell() }` shape below is *not* asserted
>    to be API-correct; follow the pinned Jewel version's official
>    `DecoratedWindow` sample.

**Control flow** (`PrototypeMain.kt`):

```kotlin
fun main() = application {
    // Replicate Main.kt's EXACT lifecycle wiring (verify against Main.kt):
    val lifecycle = LifecycleRegistry()
    val root = RootComponent(
        componentContext = DefaultComponentContext(lifecycle),
        meetings    = FakeMeetingRepository(),
        recording   = FakeRecordingRepository(),
        settings    = FakeSettingsRepository(),
        diagnostics = FakeDiagnosticsRepository(),
    )                                  // NO initCore(), NO System.setProperty libraryOverride
    // e.g. LifecycleController(lifecycle, windowState) inside the window, or
    // lifecycle.resume() — copy whatever Main.kt actually does, or VMs won't init.
    PrototypeRoot(root, ::exitApplication)
}
```

`PrototypeRoot` owns the **runtime variant state** and the **window choice**:

```kotlin
const val USE_DECORATED_WINDOW = true   // macOS fallback switch (D4)

@Composable
fun ApplicationScope.PrototypeRoot(root: RootComponent, onExit: () -> Unit) {
    var variant by remember { mutableStateOf(UiVariant.Jewel) }   // default = Jewel (D5)
    val toggle: @Composable () -> Unit = { VariantToggleBar(variant) { variant = it } }

    if (variant == UiVariant.Jewel && USE_DECORATED_WINDOW) {
        DecoratedWindow(onCloseRequest = onExit, title = "Meeting Assistant — Jewel") {
            JewelTheme {                         // IntUiTheme dark
                Column {
                    TitleBar { Text("Meeting Assistant") }   // Jewel chrome
                    toggle()
                    JewelAppShell(root)          // Jewel sidebar + content
                }
            }
        }
    } else {
        Window(onCloseRequest = onExit,
               title = if (variant == UiVariant.Jewel) "Meeting Assistant — Jewel"
                       else "Meeting Assistant — Material") {
            Column {
                toggle()
                when (variant) {
                    UiVariant.Material -> AppContent(root)        // EXISTING, unedited
                    UiVariant.Jewel    -> JewelTheme { JewelAppShell(root) }
                }
            }
        }
    }
}
```

Switching `variant` recomposes and (when crossing the `DecoratedWindow`/`Window`
boundary) recreates the window **in-process** — a brief re-create, **no JVM
restart** (this is the precise meaning of D5's "no restart"; the window
remount re-runs `LaunchedEffect`s, so the Settings/list fixtures reload —
harmless). If the owner finds the flicker jarring, the documented
single-window alternative is: always host in `DecoratedWindow` and simply
**hide `TitleBar` when variant == Material**.

`VariantToggleBar` is a small always-visible strip with **two** controls:
- a `{Material | Jewel}` segmented control (the variant), and
- a `{Populated · Empty · Loading · Error}` segmented control (the **data
  state**) that drives the fake repositories so the owner can actually see
  the empty "Нет встреч" / loading / error renderings in both variants.
  If wiring the data-state picker into the VM `StateFlow` proves fiddly,
  fall back to a documented **compile-time fixture flag**
  (`SampleData.state = …`) and drop the picker — note which in `PROTOTYPE.md`.

**Why reuse `AppContent` for Material:** it already *is* the production
Material rendering of both screens; the comparison must be against the real
thing, and reusing it guarantees zero drift and zero edits to `shared`.
**Scope caveat:** verify whether `AppContent`/`Sidebar`/`ContentPane` rely on
`FrameWindowScope` or window CompositionLocals (e.g. a `MenuBar`). If they do,
call `AppContent(root)` **directly inside the `Window {}` lambda** (which *is*
a `FrameWindowScope`) rather than wrapping it in an outer non-window `Column`;
restructure `PrototypeRoot` accordingly.

**Jewel screens mirror content & density (D6), not pixels:** same data,
same controls, same sections; desktop-idiomatic sizing (compact ~28–32.dp
controls, ~6–8.dp spacing, ~13–14.sp text — Jewel's Int UI gives this for
free). Russian labels copied verbatim from the Material screens.

## 3. Version pins (starting point — verify by Gradle resolution)

These are the research-derived coherent set. The implementer **must confirm
each resolves**; if a newer patch is required, bump it (D1/D5 authorize this)
and record the final numbers in the README.

| Artifact | From | To (target) |
|---|---|---|
| Compose Multiplatform (`org.jetbrains.compose`) | 1.7.3 | **1.10.0** |
| Kotlin (`org.jetbrains.kotlin.multiplatform`) | 2.0.21 | **2.1.20** (any 2.1.x CMP 1.10 accepts) |
| Compose compiler plugin (`org.jetbrains.kotlin.plugin.compose`) | 2.0.21 | **= Kotlin version** |
| Jewel standalone (`org.jetbrains.jewel:jewel-int-ui-standalone`) | — | **0.36-** line for CMP 1.10 (use the build matching IJP 2026.1.x) |
| Jewel decorated window (`org.jetbrains.jewel:jewel-int-ui-decorated-window`) | — | same as Jewel standalone |
| Decompose (`com.arkivanov.decompose`) | 3.2.2 | **3.3.x** (Kotlin 2.1-compatible) |
| Decompose extensions-compose | 3.2.2 | same as Decompose |
| kotlinx-coroutines | 1.8.1 | **1.9.0** (Kotlin 2.1-friendly) |
| markdown-renderer-m3 | 0.30.0 | bump to a CMP-1.10 build **or** stub the consuming screen (D7) |
| Gradle wrapper | 8.10 | bump only if Kotlin 2.1.x demands it (e.g. ≥8.11) |

Repos — add to **both** `pluginManagement` (if needed) and
`dependencyResolutionManagement` in `ui-compose/settings.gradle.kts`:

```kotlin
maven("https://packages.jetbrains.team/maven/p/kpm/public/")
// keep existing: mavenCentral(), google(),
//   maven("https://maven.pkg.jetbrains.space/public/p/compose/dev")
```

> Jewel artifacts are suffixed by IntelliJ-platform build (e.g.
> `0.36-…`). Pick the published coordinate whose release notes state
> **CMP 1.10.0**. If `0.36` is unavailable, the nearest CMP-1.10 Jewel release
> is acceptable — record which.

## 4. Phased execution

### Phase 0 — Branch + toolchain upgrade + Jewel smoke (HARD GATE)

Goal: prove the *chosen path* (upgraded toolchain + real Jewel) actually
builds and renders before any screen work. This replaces the PRD's now-moot
"discover if Jewel is compatible" spike.

1. `git checkout feat/compose-desktop-rewrite` → `git checkout -b
   proto/jewel-look-feel`. Confirm `git status` clean; no other branch
   touched. **Do NOT push** — the throwaway branch stays local; commits are
   the deliverable. Push only if the owner explicitly asks.
2. **Pin the Gradle wrapper up front** (don't discover it's too old
   mid-build): set `ui-compose/gradle/wrapper/gradle-wrapper.properties` to a
   version that supports Kotlin 2.1.x (e.g. **8.13**); decide now, not
   reactively.
3. Edit `ui-compose/gradle/libs.versions.toml`: bump kotlin, compose,
   compose-compiler plugin, decompose, coroutines per §3; add `jewel`
   version + library aliases.
4. Edit `ui-compose/settings.gradle.kts`: add the kpm/public Maven repo.
5. Add Jewel deps to `ui-compose/desktopApp/build.gradle.kts` `desktopMain`
   (`jewel-int-ui-standalone`, `jewel-int-ui-decorated-window`).
6. **Material-baseline integrity check (gate-critical):** confirm
   `compose.materialIconsExtended` still resolves on CMP 1.10 (it has been
   repackaged/deprecated across CMP releases). If it does **not** resolve,
   the *existing Material screens* break and the whole comparison is void —
   fix it here by adding the standalone `material-icons-extended` coordinate
   or replacing extended-icon usages. A broken Material baseline is a Phase-0
   failure, not a Phase-1 surprise.
7. **Decompose ↔ Compose alignment:** inspect the dependency tree for the
   Compose version Decompose `extensions-compose` pulls; if it conflicts with
   CMP 1.10 (Skiko/runtime-skew vector), force/align it (branch is
   throwaway — forcing is acceptable).
8. Resolve the build: `cd ui-compose && ./gradlew :desktopApp:dependencies
   --configuration desktopRuntimeClasspath` then a clean
   `:desktopApp:compileKotlinDesktop`. Fix version skew iteratively (align
   `org.jetbrains.compose` vs any `androidx.compose`). Under **D7**, if a
   non-target screen (most likely the markdown-renderer Protocol-detail
   screen) blocks compilation: either bump markdown-renderer to a CMP-1.10
   build, or replace that screen's body with a `// PROTOTYPE STUB` placeholder
   composable and note it in the README. **Never** alter the two target
   screens or VM/domain logic.
9. Write a throwaway `PrototypeSmoke.kt` (a `fun main() = application { ... }`
   that renders, **using the pinned Jewel version's official sample
   structure**, a `DecoratedWindow` + `TitleBar` + Jewel theme + a Jewel
   button + a Russian-text label). Owner runs it (command in §5) and confirms
   a Jewel window with a Jewel-styled button appears **and the Russian label
   renders without tofu** (Cyrillic glyph check — see Phase 2).
10. **Gate decision:**
   - Renders → delete the smoke file, proceed to Phase 1.
   - Cannot make Jewel run even after reasonable upgrade effort (the only
     true blocker) → **documented last-resort fallback**: implement the same
     two screens with a *custom compact non-Material theme* on the upgraded
     CMP (no Jewel), **following the concrete recipe in
     `claude-research.md` §B4** (token object: ~28–32.dp controls, ~6–8.dp
     spacing, ~13–14.sp text, explicit hover, system font) — it is not a
     cliff, the recipe exists. Note prominently in the README. The verdict
     is still produced. (Residual PRD fallback, demoted to last-resort by
     the owner's decision.)

Deliverable: branch with upgraded, resolving build + a rendered Jewel smoke
window (or recorded last-resort decision).

### Phase 1 — Fakes + dev entry (no FFI), Material variant runs

Goal: the prototype launches with the **existing Material screens** on fake
data, no Rust, no `initCore`.

0. **Read before writing (do not skip):** open `RootComponent.kt` and every
   repository interface in `commonMain`; record the exact constructor
   signature, suspend-function signatures (params, return types, nullability,
   thrown exceptions), and the public ViewModel accessors. Open `Main.kt` and
   record its exact Decompose lifecycle wiring. The fakes and
   `PrototypeMain.kt` are written to *these* signatures, not the pseudocode
   in §2.
1. `prototype/fakes/SampleData.kt`: static fixtures —
   - `sampleMeetings`: ≥3 `Meeting` covering all states (has both, has
     transcript only, has neither) + an easy way to produce the empty list to
     exercise "Нет встреч".
   - `sampleSettings`: a fully populated `Settings` (api key set, source
     "mixed", echo on, a default template, ru language, beamSize 3, nThreads
     0, plausible paths).
   - `sampleTemplates`: ≥2 names. `sampleModels`: ≥2 `WhisperModel`
     (name, sizeMb, description).
2. `FakeMeetingRepository` / `FakeSettingsRepository` implement the
   `commonMain` repository interfaces returning the fixtures (suspend
   functions return immediately; `list()` returns `sampleMeetings`;
   settings get/set are in-memory; templatesList/modelsList return fixtures).
   `FakeRecordingRepository` / `FakeDiagnosticsRepository`: minimal no-op
   stubs sufficient for `RootComponent` construction.
3. `UiVariant.kt`, `PrototypeRoot.kt` (Material branch only for now wired:
   `when` renders `AppContent(root)`; Jewel branch renders a "TODO" box),
   `PrototypeMain.kt` per §2.
4. **Run path — primary approach:** in `desktopApp/build.gradle.kts` set the
   Compose Desktop `application.mainClass = "PrototypeMainKt"` **on the
   branch**. This is the supported single-mainClass path; it is harmless here
   because the branch is throwaway and the in-app toggle still reaches the
   Material variant, so the production `Main` does not need to be runnable on
   this branch. Run with `./gradlew :desktopApp:run` (no `-Drust.target.dir`,
   no FFI). A dedicated `JavaExec`/`runPrototype` task against the KMP
   runtime classpath is an **optional nicety only** — do not block on it; the
   raw classpath wiring is a known time sink and is not a required
   deliverable.
5. Owner runs it: a window opens, **Material** screens render from fake data
   with no Rust build and no `ANTHROPIC_API_KEY`.

Deliverable: prototype launches on a bare JDK; Material MeetingList +
Settings render from fakes; navigation works.

### Phase 2 — JewelMeetingListScreen

Goal: Jewel rendering of the Sidebar/MeetingList, content & density faithful.

1. `prototype/jewel/JewelTheme.kt`: wrap content in Jewel's dark `IntUiTheme`
   (`org.jetbrains.jewel.intui.standalone.theme.*`); expose any shared spacing
   tokens. **Do not** nest Material `MaterialTheme` inside it (research B2).
   **Cyrillic check:** every label in both screens is Russian. Confirm
   Jewel's default Int UI font renders Cyrillic. If it shows tofu/boxes,
   provide a Cyrillic-capable `FontFamily` (e.g. a bundled Inter/Noto or a
   system font) to the Jewel `TextStyle`/`ThemeDefinition`. A font bug here
   would invalidate the owner's verdict, so treat correct Cyrillic rendering
   as a Phase-2 exit requirement.
2. `prototype/jewel/JewelAppShell.kt`: a `Row` mirroring `AppContent`'s
   layout — Jewel sidebar (~280.dp) | divider | content pane that switches on
   `root.screen` (subscribe via Decompose `subscribeAsState()` exactly like
   `AppContent`). Content pane renders `JewelMeetingListScreen` /
   `JewelSettingsScreen` (and a simple Jewel placeholder for any other
   `Screen` value so navigation never crashes).
3. `JewelMeetingListScreen.kt`: consume `root.meetingListViewModel.state`
   (same `MeetingListState`). Render with Jewel components:
   - Header row: title "Встречи" + Jewel icon buttons for add & refresh
     (wire to the same VM/nav callbacks the Sidebar uses).
   - `LazyColumn` of rows: meeting name, `formatDate(createdAt)`, the
     "Транскрипт"/"Протокол" status chips, Jewel selection/hover styling;
     click → `root.onMeetingSelected(...)`.
   - Empty state "Нет встреч"; `Loading` → Jewel progress;
     `Error` → message.
   - Footer: Jewel icon buttons → `root.onSettingsRequested()` etc.

Deliverable: `JewelMeetingListScreen` compiles and renders from fakes.

### Phase 3 — JewelSettingsScreen (form-density stress point)

Goal: Jewel rendering of every Settings control (the hardest test of
"desktop vs mobile").

Mirror `SettingsScreen.kt` section-for-section using Jewel equivalents (use
Jewel `TextField`, `RadioButtonChain`/segmented equivalent, `Checkbox`/
toggle, `Dropdown`/combo, sliders, `DefaultButton`/`OutlinedButton`; for any
control Jewel lacks, use the closest Jewel primitive and note it):

1. Load fixtures via the same repository calls the Material screen uses
   (`settings.get()`, `templatesList()`, `modelsList()`), in a
   `LaunchedEffect`, into local state — copy the Material screen's state
   shape so behavior matches.
2. Sections, in order, with verbatim Russian labels:
   API key (secret field + show/hide + validation text) · Recording (source
   segmented {Микрофон,Система,Оба} + echo toggle) · Protocol template
   dropdown · Transcription (language segmented {Русский,English,Авто} +
   accuracy slider 1–5 + threads slider 0–16) · Storage (path fields with the
   same `/`/`~` validation + model dropdown `"name · sizeMb MB"` + "Другой
   путь…" custom path + prompts dir) · Save button
   (`enabled = pathsValid && apiKeyValid`) + Jewel feedback (snackbar-equivalent
   or inline status).
3. Toolbar: Jewel header "Настройки" + back affordance →
   `root.onBackToList()`.

Deliverable: `JewelSettingsScreen` compiles and renders all controls from
fakes.

### Phase 4 — Toggle, decorated window, macOS fallback, README

Goal: a usable side-by-side comparison build + handoff doc.

1. Finalize `VariantToggleBar`: always-visible strip, two states
   "Material | Jewel", reflects/sets `variant`. Verify a live flip both ways,
   on both screens, with navigation, **no restart**.
2. Wire `USE_DECORATED_WINDOW` (D4): Jewel variant → `DecoratedWindow` +
   `TitleBar`; Material → standard `Window`. Confirm the
   variant-crossing window recreation works in-process. Document that flipping
   `USE_DECORATED_WINDOW = false` keeps Jewel content in a standard `Window`
   (the macOS fallback) if the decorated window misbehaves on the owner's Mac.
3. Write **`ui-compose/PROTOTYPE.md`** (the branch README), containing:
   - One-paragraph purpose + the "throwaway, never merge" warning.
   - Exact build/run commands (§5), incl. that **no Rust build / no
     `ANTHROPIC_API_KEY` / no `run-compose.sh`** is needed.
   - Final pinned versions actually used: CMP, Kotlin, compose-compiler,
     Jewel coordinate, Decompose, coroutines, Gradle; **every** dependency
     bumped vs `main`; any screen stubbed/excluded under D7.
   - How the toggle works; default = Jewel; the `USE_DECORATED_WINDOW`
     macOS fallback.
   - Whether the last-resort custom-theme fallback was taken (Phase 0).
   - The **verdict template** (Phase 5).

Deliverable: runnable comparison build + `PROTOTYPE.md`.

### Phase 5 — Verdict & decision close-out (owner-driven)

1. Owner builds, runs, toggles both screens, and fills the verdict template
   in `PROTOTYPE.md`:
   ```
   MeetingList — verdict: [ desktop / still mobile / inconclusive ] — notes:
   Settings    — verdict: [ desktop / still mobile / inconclusive ] — notes:
   Overall stack decision: [ stay on Compose + restyle / migrate to Qt ]
   ```
2. Record the outcome in project memory:
   - **(a) "theming fixes it"** → stay on Compose; open a *real* work item to
     restyle in-place (Jewel or a custom desktop theme) and **drop Qt**.
   - **(b) "still not desktop enough"** → Qt's look argument stands; open a
     Qt-migration plan.
3. Branch remains **unmerged**. Update the memory note related to
   `project_goal_shipping` / the Qt-vs-Compose open question with the decision
   and date.

Deliverable: recorded decision; throwaway branch left in place, never merged.

## 5. Build & run commands (documented for the owner)

```bash
# 1. Get on the branch
git checkout proto/jewel-look-feel

# 2. Resolve / sanity-compile the upgraded toolchain + Jewel
cd ui-compose
./gradlew :desktopApp:compileKotlinDesktop          # must succeed

# 3. Run the prototype (NO Rust build, NO initCore, NO ANTHROPIC_API_KEY)
#    mainClass is set to PrototypeMainKt on this throwaway branch:
./gradlew :desktopApp:run                           # fake data, opens the window
#    (optional, only if the JavaExec nicety was added: :desktopApp:runPrototype)

# Phase-0 smoke only (temporary, if the smoke main was used):
# point application.mainClass at PrototypeSmokeKt, ./gradlew :desktopApp:run
```

The assistant cannot launch a desktop GUI; it delivers the branch in a
**compiling** state with these exact commands. The owner performs the visual
run and records the verdict.

## 6. Constraints & isolation rules

- **Do not edit** `shared` ViewModels/domain or the Rust core. Permitted
  edits outside the `prototype` package: (i) `libs.versions.toml`,
  `settings.gradle.kts`, `desktopApp/build.gradle.kts` for the toolchain/dep
  bumps + run task (D1); (ii) **only if the upgrade forces it**, a minimal
  `// PROTOTYPE STUB` body swap on a *non-target* screen (D7) — logic
  untouched, noted in README.
- The Material comparison is the **unmodified** `AppContent(root)`.
- Dark theme both variants (app default) for a fair comparison.
- No persistence of the toggle; in-memory only.
- New compiler warnings inherent to the CMP/Kotlin upgrade are acceptable on
  this throwaway branch (note them in README); do not introduce *new* logic
  warnings in the Material paths. (Section files must carry this **softened**
  wording, not the PRD's absolute "no new warnings".)
- **Verify before writing:** fakes and `PrototypeMain` are written against
  the *actual* `RootComponent`/repository signatures and `Main.kt`'s actual
  lifecycle wiring, read first (Phase 1 step 0) — never against §2 pseudocode.
- If `AppContent` requires `FrameWindowScope`, call it inside the `Window {}`
  lambda, not an outer `Column` (see §2 scope caveat).
- **Git:** the branch stays **local**; deliverable = commits on
  `proto/jewel-look-feel`. Do **not** push unless the owner explicitly asks.

## 7. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Toolchain upgrade cascades (Decompose/markdown/coroutines vs Kotlin 2.1 + CMP 1.10) | Phase 0 is a hard gate before any screen work; D5 authorizes bumping anything; D7 authorizes stubbing the markdown Protocol screen. |
| Skiko/Compose runtime skew after upgrade | Pin the coherent set in §3; verify with the Phase-0 smoke window before screens. |
| Jewel cannot run even upgraded (only true blocker) | Documented last-resort: custom compact non-Material theme on upgraded CMP, same two screens & toggle; verdict preserved; recorded in README. |
| macOS `DecoratedWindow` quirks (no native traffic-lights, etc.) | `USE_DECORATED_WINDOW=false` fallback keeps Jewel content in a standard `Window`; documented. |
| FFI/JDK env friction | Eliminated by D3 — fake data, no `initCore`, no dylib; runs on bare JDK. |
| Scope creep to "make it production-ready" | Hard boundary: two screens, visual only, branch never merged, README states it. |
| Owner-only GUI verification | Assistant delivers compiling branch + exact commands + verdict template; owner runs & records. |
| Window recreation on variant flip looks jarring | Acceptable for a dev tool (in-process, no JVM restart); if disruptive, host both in `DecoratedWindow` and hide `TitleBar` in Material mode — documented option. |
| **Decompose lifecycle not resumed → blank screens** | Phase 1 step 0 reads `Main.kt`'s lifecycle wiring and `PrototypeMain` replicates it verbatim; smoke + Material-on-fakes run proves VMs init before Jewel work. |
| **`compose.materialIconsExtended` removed in CMP 1.10 → Material baseline breaks** | Gate-critical Phase-0 step 6 resolves/repairs it before any screen work. |
| **Decompose ↔ CMP Compose-version skew** | Phase-0 step 7 inspects & force-aligns Decompose's Compose pull. |
| **Cyrillic tofu in Jewel font** | Smoke step renders a Russian label; Phase-2 exit requires correct Cyrillic; mitigation = supply a Cyrillic `FontFamily` to the Jewel theme. |
| **Repository/RootComponent signatures differ from assumption** | Phase 1 step 0 "read before write" mandatory. |

## 8. Acceptance criteria

**Functional**
- [ ] `proto/jewel-look-feel` exists off `feat/compose-desktop-rewrite`; not
      merged; **proven**: `main` and other branches have no new commits
      (`git log` / branch diff check). Branch is **local-only** (not pushed
      unless owner asked).
- [ ] Upgraded toolchain resolves; `compose.materialIconsExtended` (Material
      baseline) intact on CMP 1.10; Phase-0 Jewel smoke renders **with
      Cyrillic text correct** (or last-resort §B4 fallback recorded).
- [ ] MeetingList renders in **Material (existing)** and **Jewel** variants
      from fake data; Cyrillic correct in both.
- [ ] Settings renders in **Material (existing)** and **Jewel** variants
      from fake data — every control present; Cyrillic correct in both.
- [ ] Visible in-window toggle flips Material↔Jewel (window re-create in
      process, **no JVM restart**), **default Jewel**; navigation works in
      both.
- [ ] Loading / Error / empty ("Нет встреч") renderings are reachable — via
      the data-state picker, **or** documented compile-time fixture flag if
      the picker was dropped.
- [ ] Jewel variant uses `DecoratedWindow` + `TitleBar` (or documented
      `USE_DECORATED_WINDOW=false` macOS fallback keeping Jewel content).
- [ ] Runs via `./gradlew :desktopApp:run` on a bare JDK — no Rust build, no
      `initCore`, no API key.

**Quality**
- [ ] Prototype isolated in `prototype` package; `shared` VMs/domain + Rust
      core unedited except sanctioned Gradle/run/stub changes.
- [ ] `:desktopApp:compileKotlinDesktop` succeeds on the owner's machine.
- [ ] `PROTOTYPE.md` documents build/run, final version pins, every dep
      bumped, any stubbed screen, the macOS fallback, and the verdict
      template.

**User**
- [ ] Owner launches and toggles both screens with no further instruction.
- [ ] Owner records an explicit per-screen verdict.
- [ ] Decision recorded in project memory; Qt-vs-Compose question closed;
      branch left unmerged.
