# Synthesized Specification — Desktop Look-and-Feel (Jewel) Prototype

Synthesis of: `prd-v1.0.md` (initial) + `claude-research.md` (codebase + web)
+ `claude-interview.md` (8 answered questions). Where the interview/research
contradicts the PRD, **the interview wins** and the divergence is called out.

---

## 1. Objective (unchanged from PRD)

A throwaway, dev-only prototype on a dedicated branch that re-implements two
existing screens (MeetingList, Settings) with JetBrains **Jewel**, switchable
at runtime against the current Material 3 versions, so the owner can render a
subjective verdict: **does the app read as a desktop app, not mobile?** The
outcome closes the Qt-vs-Compose stack decision. The branch is never merged.

## 2. Decisions that override the PRD

The PRD assumed Jewel might resolve against the current toolchain and made
"no toolchain change" a hard constraint with a custom-theme fallback. Research
proved that assumption false; the owner chose to **upgrade the toolchain
instead of falling back**. Binding decisions:

| # | Decision | Source |
|---|----------|--------|
| D1 | **Upgrade the toolchain on the throwaway branch**: Compose Multiplatform 1.7.3 → **1.10.0**, Kotlin 2.0.21 → **2.1.x**, matching `compose-compiler`. Bump Decompose / markdown-renderer-m3 / coroutines / anything else as needed to compile & run. PRD's "must build with CMP 1.7.3 / Kotlin 2.0.21" is **void on this branch**. | Q1, Q5 |
| D2 | **Use real Jewel**: `org.jetbrains.jewel:jewel-int-ui-standalone` + `jewel-int-ui-decorated-window`, the 0.35+/0.36 line that targets CMP 1.10.0, from repo `https://packages.jetbrains.team/maven/p/kpm/public/`. No custom-theme fallback unless Jewel itself cannot run on the upgraded toolchain. | Q1, D1 |
| D3 | **Fake data, no FFI**: fake `MeetingRepository` / `SettingsRepository` with representative static data; a dev composition root that does **not** call `initCore()` and does **not** load the Rust dylib. Prototype runs on a bare JDK. | Q2 |
| D4 | **Jewel `DecoratedWindow` + `TitleBar`** for the Jewel variant (Material variant keeps standard `Window`). macOS standard-window fallback **keeping Jewel content** only if the decorated window misbehaves on the owner's machine. | Q6 |
| D5 | **Visible in-window toggle; default = Jewel on launch.** Flips Material↔Jewel live, no restart. MeetingList↔Settings navigation works in both variants. | Q3 |
| D6 | **Fidelity = content & density, not pixels.** Same fields/controls/data, desktop-idiomatic sizing/spacing. Scope strictly = MeetingList + Settings. | Q4 |
| D7 | **Only the comparison must compile/run.** If an unrelated screen blocks the CMP/Kotlin upgrade, it may be excluded/stubbed on this branch. | Q7 |
| D8 | **Branch `proto/jewel-look-feel` off `feat/compose-desktop-rewrite`**, never merged. Assistant delivers a building branch + exact `./gradlew` commands + README; **owner runs the GUI and records the verdict** (assistant cannot launch a desktop UI). | Q8 |

## 3. Codebase facts the implementation must respect

(From `claude-research.md` Part A — file:line anchors.)

- **Composition root**: `ui-compose/desktopApp/src/desktopMain/kotlin/Main.kt`
  builds `RootComponent` after a blocking `initCore(AppConfig)` FFI call, then
  renders `AppContent(root)`. The toggle/variant + dev (no-FFI) entry point
  belong here.
- **Navigation**: `ui-compose/shared/.../ui/navigation/RootComponent.kt` holds
  4 repository interfaces + `_screen: MutableValue<Screen>` (sealed `Screen`),
  ViewModels via `instanceKeeper.getOrCreate`. Repository **interfaces** live
  in `commonMain` → faking is clean.
- **Layout shell**: `ui-compose/shared/.../ui/AppContent.kt` wraps content in
  `AppTheme { }` (Material 3, hardcoded at ~line 27) then `Row`: Sidebar
  (280.dp) | divider | ContentPane (per-screen `when`).
- **MeetingList** is really the **Sidebar** (`ui/Sidebar.kt` 24–93):
  header "Встречи" + add/refresh icons, `LazyColumn` of `MeetingListItem`
  (name `bodyMedium`, `formatDate(createdAt)` `labelSmall`, conditional
  "Транскрипт"/"Протокол" chips, selected highlight), empty state "Нет встреч",
  footer Settings/Diagnostics icons. State =
  `MeetingListState{Loading|Success(List<Meeting>)|Error}`.
  `Meeting{id,name,audioPath,hasTranscript,hasProtocol,createdAt:Long}`.
- **Settings** (`ui/screens/SettingsScreen.kt` 27–429): `Scaffold` +
  `SnackbarHost`, `SettingsToolbar` (TopAppBar, title "Настройки", back),
  `SettingsForm` sections: API key (`OutlinedTextField` w/ password toggle +
  validation), Recording (`SegmentedButton` source + `Switch` echo),
  Protocol template (`ExposedDropdownMenuBox`), Transcription (language
  segmented + 2 `Slider`s), Storage paths (`PathField`s + model
  `ExposedDropdownMenuBox` + custom path), Save `Button`
  (`enabled = pathsValid && apiKeyValid`, snackbar). `Settings` =
  `paths{model,db,meetingsDir,prompts}`, `anthropicApiKey`,
  `recording{source,echoCancel}`, `defaultTemplate`,
  `transcriber{language,beamSize 1–5,nThreads 0=auto}`.
- **Theme**: `ui/theme/AppTheme.kt` hand-rolled dark/light Material color
  schemes (primary dark `0xFF4FA3E0`); app pervasively uses
  `MaterialTheme.colorScheme/typography` and Material components. A Jewel
  screen set must re-implement these per-component (do **not** nest
  `MaterialTheme` inside `IntUiTheme` — research B2).
- **Build**: `gradle/libs.versions.toml` (kotlin 2.0.21, compose 1.7.3,
  decompose 3.2.2, markdown-renderer 0.30.0, coroutines 1.8.1, jna 5.15.0);
  repos in `settings.gradle.kts`; native-lib copy tasks + `-Drust.target.dir`
  jvmArgs in `desktopApp/build.gradle.kts`. Gradle wrapper 8.10.

## 4. Functional requirements

1. Throwaway branch `proto/jewel-look-feel` off `feat/compose-desktop-rewrite`;
   `main` and other branches untouched; never merged.
2. Toolchain upgraded on the branch (D1): CMP 1.10.0, Kotlin 2.1.x, matching
   compose-compiler; transitive bumps as needed; project (at least the two
   target screens + their Material variants + the dev entry) compiles.
3. Jewel dependency resolves & runs (D2). A trivial Jewel smoke screen (label
   + button inside `IntUiTheme`/`DecoratedWindow`) builds and renders — the
   Phase-1 proof, now a *proof of the chosen path*, not a discovery spike.
4. `JewelMeetingListScreen` mirrors the Sidebar's content & density (list,
   header actions, status chips, empty state, footer nav) using Jewel
   components.
5. `JewelSettingsScreen` mirrors the Settings form's content & density —
   every control enumerated in §3 has a Jewel equivalent (text field with
   secret toggle, segmented/radio source, switch, dropdowns, sliders, path
   fields, save button + feedback).
6. Fake repositories supply representative static data so both screens render
   fully without `initCore`/FFI (D3): ≥2 meetings with mixed
   transcript/protocol flags incl. an empty-state toggle path; a populated
   `Settings` + ≥2 templates + ≥2 whisper models.
7. `UiVariant { Material, Jewel }` read at the composition root; a visible
   in-window control flips it live with **no restart**; **default = Jewel**
   (D5). Navigation MeetingList↔Settings works in both variants.
8. Jewel variant uses `DecoratedWindow` + `TitleBar`; Material variant uses
   standard `Window` (D4). macOS standard-window fallback keeping Jewel
   content is implemented/documented as a switch if needed.
9. Branch `README` (or `PROTOTYPE.md`) states: what was built, exact
   `./gradlew` build/run commands (dev no-FFI entry), the pinned
   Jewel/CMP/Kotlin versions, every dependency bumped, any screen
   excluded/stubbed (D7), the macOS decorated-window fallback toggle, and a
   per-screen verdict template for the owner.

## 5. Quality / constraints

- Prototype code isolated in its own package(s)
  (e.g. `ui/prototype/jewel/…`, `ui/prototype/fakes/…`, dev entry point). **No
  edits to `shared` ViewModels/domain or the Rust core** beyond: the
  toolchain/dependency bumps in Gradle/version catalog (D1), the minimal
  `UiVariant` plumbing at the composition root, and any unrelated-screen
  stub strictly required to make the upgraded branch compile (D7, noted in
  README).
- No new compiler warnings in the existing Material code paths beyond those
  inherent to the CMP/Kotlin upgrade (the upgrade itself may surface
  deprecations — acceptable on a throwaway branch, noted in README).
- Build succeeds on the owner's macOS with the documented commands. Since
  data is faked and FFI is skipped, the run path does **not** require the
  Rust build or `run-compose.sh`; it is a pure Gradle desktop run of a dev
  entry point.
- Dark theme by default for both variants (the app defaults to dark) so the
  comparison is fair; Jewel uses its Int UI dark theme.

## 6. Risks & mitigations (updated)

| Risk | Mitigation |
|------|------------|
| **Toolchain upgrade cascades** (Decompose 3.2.2 / markdown-renderer 0.30.0 / coroutines vs Kotlin 2.1 + CMP 1.10) | D5: bump whatever's needed; D7: stub/exclude unrelated screens (esp. the markdown-renderer ProtocolDetail) so only the comparison must compile. Sequence the upgrade *before* any Jewel screen work and gate on a smoke build. |
| **Skiko / runtime skew after upgrade** | Pin a coherent CMP+Kotlin+compose-compiler+Jewel set from research (CMP 1.10.0 ↔ Jewel 0.35+/0.36 ↔ Kotlin 2.1.x). Verify with the trivial Jewel smoke screen before screens. |
| **Jewel can't run even after upgrade** (only true blocker) | Documented last-resort fallback: custom compact non-Material theme on the upgraded toolchain (still CMP, no Jewel) — preserves the verdict. Recorded in README if taken. |
| **macOS DecoratedWindow quirks** | Standard-window-keeping-Jewel-content fallback wired behind a flag/const; documented. |
| **FFI/JDK env friction** | Removed for this test by D3 (fake data, no `initCore`, no dylib). |
| **Scope creep to production** | Hard boundary: two screens, visual only, branch never merged, README says so. |
| **Owner-only GUI verification** | Assistant delivers building branch + exact commands + verdict template; owner runs & records. |

## 7. Acceptance criteria

Functional:
- [ ] Branch `proto/jewel-look-feel` exists off `feat/compose-desktop-rewrite`;
      other branches untouched; not merged.
- [ ] Toolchain upgraded; smoke Jewel window builds & renders.
- [ ] MeetingList renders in both Material (existing) and Jewel variants.
- [ ] Settings renders in both Material (existing) and Jewel variants.
- [ ] Visible in-window toggle flips Material↔Jewel live, no restart, default
      Jewel; navigation works in both.
- [ ] Jewel variant uses DecoratedWindow + TitleBar (or documented macOS
      fallback).
- [ ] Runs with fake data on a bare JDK — no Rust build, no `initCore`.

Quality:
- [ ] Prototype isolated in its own package(s); `shared` VMs/domain + Rust
      core unedited except the sanctioned Gradle/UiVariant/stub changes.
- [ ] Build succeeds on owner's machine with documented `./gradlew` commands.
- [ ] README documents build/run, version pins, every dep bumped, excluded
      screens, macOS fallback, and the verdict template.

User:
- [ ] Owner launches the app and toggles both screens without further
      instruction.
- [ ] Owner records an explicit per-screen verdict ("desktop, not mobile" vs
      "still mobile / not enough").
- [ ] Verdict recorded in project memory so the Qt-vs-Compose decision closes.

## 8. Out of scope (unchanged)

Production integration; other screens beyond the two (except cheap stubs to
keep the build green); behavior/logic changes; persisting the toggle;
packaging/signing; macOS system audio; the actual Qt-vs-Compose decision
(that is the *outcome*, not part of this work).
