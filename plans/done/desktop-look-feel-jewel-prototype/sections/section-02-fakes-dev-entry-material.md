# Section 02: Fakes + Dev Entry Point + Material Variant Runs

## Background

**Meeting Assistant** is a Rust + Kotlin Multiplatform Compose Desktop app
(`/ui-compose`). This is a **throwaway, never-merged** prototype: re-render
two screens (MeetingList, Settings) with JetBrains Jewel and toggle them at
runtime against the existing Material 3 screens, so the owner can decide
"desktop vs mobile" and close a Qt-vs-Compose stack question. Section 01 (the
hard gate) has already upgraded the toolchain (CMP 1.10 / Kotlin 2.1.x) and
proved Jewel renders.

This section makes the prototype **launch and render the existing Material
screens on fake data, with no Rust and no FFI**. The owner's binding decision
**D3**: fake repositories with static data; a dev entry point that never
calls `initCore()` and never loads the Rust dylib — it runs on a bare JDK.
This both removes the FFI/native env risk from a purely visual test and gives
the Material baseline (the comparison target) before any Jewel screen work.

The Material variant of both screens **is the existing, unmodified
`AppContent(root)`** — reusing it guarantees zero drift and zero edits to
`shared`.

## Requirements

When this section is complete:

- Fake implementations of all four repository interfaces exist, returning
  representative static data (populated / empty / loading / error fixtures).
- A dev entry `PrototypeMain.kt` constructs `RootComponent` with the fakes,
  replicating `Main.kt`'s Decompose lifecycle wiring **verbatim**, with **no
  `initCore()` and no dylib**.
- `UiVariant` enum + `PrototypeRoot` exist; the Material branch renders the
  **unmodified** `AppContent(root)`; the Jewel branch is a temporary
  placeholder.
- The branch's `application.mainClass` points at `PrototypeMainKt`.
- `cd ui-compose && ./gradlew :desktopApp:run` opens a window showing the
  Material MeetingList + Settings from fake data; navigation works; no Rust
  build, no `ANTHROPIC_API_KEY`.

## Dependencies

- **Requires:** section 01 (toolchain upgraded, Jewel gate passed).
- **Blocks:** sections 03, 04, 05 (Jewel screens, toggle).

## Implementation Details

### Step 0 — Read before writing (DO NOT SKIP)

The Kotlin in `claude-plan.md` §2 is **illustrative pseudocode**. Before
writing anything, read and record the *actual* signatures:

- `ui-compose/shared/src/commonMain/kotlin/ui/navigation/RootComponent.kt` —
  exact constructor signature (param order/names/types), the sealed `Screen`
  type and its cases, the nav methods (`onMeetingSelected`,
  `onSettingsRequested`, `onBackToList`, …), and the **public** ViewModel
  accessors (`meetingListViewModel`, `recordingViewModel`, …).
- Each repository **interface** in `commonMain` referenced by `RootComponent`
  (`MeetingRepository`, `RecordingRepository`, `SettingsRepository`,
  `DiagnosticsRepository`): every method's exact signature — suspend or not,
  parameter types, return types, **nullability**, declared exceptions.
- `ui-compose/desktopApp/src/desktopMain/kotlin/Main.kt` — the **exact**
  Decompose lifecycle wiring: how `LifecycleRegistry` is created, whether it
  uses `lifecycle.resume()` or `LifecycleController(lifecycle, windowState)`
  bound inside the window, and the order of construction relative to
  `application {}` / `Window`.
- The domain types: `Meeting`
  (`id, name, audioPath, hasTranscript, hasProtocol, createdAt: Long`),
  `Settings` (`paths{model,db,meetingsDir,prompts}`, `anthropicApiKey`,
  `recording{source,echoCancel}`, `defaultTemplate`,
  `transcriber{language,beamSize 1–5,nThreads 0=auto}`), `WhisperModel`
  (`name, sizeMb, description`), template type, and `MeetingListState`
  (`Loading | Success(List<Meeting>) | Error(msg)`).

Fakes and `PrototypeMain.kt` are written against **these real signatures**,
never the pseudocode.

> **Critical footgun (why Step 0 matters):** `RootComponent` builds
> ViewModels via Decompose `instanceKeeper.getOrCreate`, and
> `MeetingListViewModel` runs `init { load() }`. If the Decompose lifecycle is
> **not resumed/bound exactly as `Main.kt` does it**, the ViewModels never
> initialize and **screens render blank**. Replicate `Main.kt`'s lifecycle
> setup verbatim.

### Step 1 — `SampleData.kt`

`ui-compose/desktopApp/src/desktopMain/kotlin/prototype/fakes/SampleData.kt`:

- `sampleMeetings`: ≥3 `Meeting` covering every state — one with both
  `hasTranscript` and `hasProtocol` true, one transcript-only, one with
  neither — plus an easy way to yield the **empty list** (to exercise the
  "Нет встреч" empty state).
- `sampleSettings`: a fully populated `Settings` — api key set
  (`sk-ant-...`), recording source `"mixed"`, echo on, a `defaultTemplate`
  set, transcriber language `ru`, beamSize 3, nThreads 0, plausible paths.
- `sampleTemplates`: ≥2 template names. `sampleModels`: ≥2 `WhisperModel`
  (name, sizeMb, description).
- A `DataState` enum `{ Populated, Empty, Loading, Error }` and a
  process-wide mutable selector (e.g. `var SampleData.state = Populated`) so
  the data-state picker (section 05) — or a documented compile-time flag —
  can switch what the fakes return. Default `Populated`.

### Step 2 — Fake repositories

In `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/fakes/`:

- `FakeMeetingRepository` implements `MeetingRepository`. `list()` returns
  `sampleMeetings` for `Populated`, empty list for `Empty`, throws/never-
  returns appropriately for `Error`/`Loading` (match the real interface's
  contract so `MeetingListViewModel` maps it to
  `Loading/Success/Error`). All suspend functions return promptly.
- `FakeSettingsRepository` implements `SettingsRepository`: `get()` returns
  `sampleSettings` (in-memory, mutable on `set()`), `templatesList()` →
  `sampleTemplates`, `modelsList()` → `sampleModels`.
- `FakeRecordingRepository`, `FakeDiagnosticsRepository`: minimal no-op stubs
  — just enough for `RootComponent` construction (return empty/default values
  from every interface method).

Every fake matches the **real interface signatures** recorded in Step 0
(suspend modifiers, nullability, return types).

### Step 3 — `UiVariant`, `PrototypeRoot`, `PrototypeMain`

- `prototype/UiVariant.kt`: `enum class UiVariant { Material, Jewel }`.
- `prototype/PrototypeRoot.kt`: an `ApplicationScope` composable owning
  variant state (`var variant by remember { mutableStateOf(UiVariant.Jewel) }`
  — default Jewel per D5) and the window. **For this section** the Jewel
  branch renders a simple placeholder ("Jewel — TODO, section 03/04"); the
  Material branch renders the **unmodified** `AppContent(root)`. Wire the
  `USE_DECORATED_WINDOW` constant and the window-choice skeleton so section 05
  only has to fill it in.
  - **Scope caveat:** check whether `AppContent` / `Sidebar` / `ContentPane`
    require `FrameWindowScope` or window CompositionLocals (e.g. a `MenuBar`).
    If they do, call `AppContent(root)` **directly inside the `Window {}`
    lambda** (which *is* a `FrameWindowScope`), not inside an outer non-window
    `Column`; structure `PrototypeRoot` accordingly.
- `desktopApp/src/desktopMain/kotlin/PrototypeMain.kt`: `fun main() =
  application { ... }` that builds `RootComponent` with the four fakes and
  **replicates `Main.kt`'s exact lifecycle wiring** (from Step 0) — but
  **without** `initCore(...)` and **without** the
  `System.setProperty(...libraryOverride...)` dylib line. Then calls
  `PrototypeRoot(root, ::exitApplication)`.

### Step 4 — Run path (primary approach)

In `ui-compose/desktopApp/build.gradle.kts` set the Compose Desktop
`application.mainClass = "PrototypeMainKt"` **on this branch**. This is the
supported single-mainClass path and is harmless here: the branch is throwaway,
never merged, and the in-app toggle still reaches Material, so the production
`Main` need not be runnable on this branch. A dedicated
`JavaExec`/`runPrototype` task against the KMP runtime classpath is an
**optional nicety only** — do not block on it (the raw classpath wiring is a
known time sink and is not a required deliverable).

### Step 5 — Owner verification

Deliver the branch compiling. The owner runs:

```bash
cd ui-compose
./gradlew :desktopApp:run
```

A window opens; the **Material** MeetingList (sidebar list) and Settings
render from fake data; navigating MeetingList ↔ Settings works. No Rust
build, no `initCore`, no `ANTHROPIC_API_KEY` required.

## Acceptance Criteria

- [ ] Step 0 done: actual `RootComponent`/repository/`Main.kt`-lifecycle
      signatures recorded; fakes/`PrototypeMain` written against them, not
      pseudocode.
- [ ] `SampleData.kt` provides populated/empty fixtures (≥3 meetings covering
      all flag combinations; full `Settings`; ≥2 templates; ≥2 models) and a
      `DataState` selector defaulting to `Populated`.
- [ ] All four repository interfaces have fake implementations matching the
      real signatures; `RootComponent` constructs cleanly with them.
- [ ] `PrototypeMain.kt` builds `RootComponent` with fakes, replicates
      `Main.kt`'s lifecycle **verbatim**, and contains **no `initCore()` and
      no dylib/libraryOverride** code.
- [ ] `UiVariant` enum + `PrototypeRoot` exist; Material branch renders the
      **unmodified** `AppContent(root)`; Jewel branch is a placeholder;
      `AppContent` scope requirement checked and honored.
- [ ] `application.mainClass = "PrototypeMainKt"` on the branch.
- [ ] `./gradlew :desktopApp:run` opens a window; Material MeetingList +
      Settings render from fake data; navigation works; **no Rust build, no
      `initCore`, no API key**.
- [ ] No edits to `shared` ViewModels/domain or the Rust core; prototype code
      isolated under `prototype/`.

## Files to Create/Modify

**Create:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/PrototypeMain.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/UiVariant.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/PrototypeRoot.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/fakes/SampleData.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/fakes/FakeMeetingRepository.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/fakes/FakeSettingsRepository.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/fakes/FakeRecordingRepository.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/fakes/FakeDiagnosticsRepository.kt`

**Modify:**

- `ui-compose/desktopApp/build.gradle.kts` — set
  `application.mainClass = "PrototypeMainKt"`.

**Do not modify:** `shared` ViewModels/domain, `AppContent`, the two target
screens, the Rust core.
