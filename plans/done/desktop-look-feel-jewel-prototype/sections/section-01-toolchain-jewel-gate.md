# Section 01: Toolchain Upgrade + Jewel Compatibility Gate

## Background

**Project.** Meeting Assistant is a desktop app: a Rust core (audio capture,
transcription, protocol generation) exposed over UniFFI to a **Kotlin
Multiplatform + Compose Desktop** UI living in `ui-compose/`. The UI today is
stock **Material 3**, and the project owner feels it "feels mobile". That single
complaint is the only surviving argument for throwing the whole stack away and
migrating to Qt.

**What this prototype is.** This is a **throwaway, never-merged** experiment —
not a feature. It re-renders two existing screens (MeetingList and Settings)
with **JetBrains Jewel** (the desktop component library that gives
IntelliJ-grade, native-feeling widgets), puts a **runtime toggle** in the window
against the current Material 3 screens, and lets the owner look at both side by
side and decide: *"this reads as a desktop app"* — or not. That verdict closes
the Qt-vs-Compose decision. **Nothing here ever ships or merges.** Fake data
only, no FFI, no Rust dylib — it runs on a bare JDK.

**Why this section (the HARD GATE).** This is **Phase 0** of the plan and it
gates everything else. The headline research finding is the reason it exists:

> **There is no JetBrains Jewel release that targets the project's pinned
> Compose Multiplatform 1.7.3 / Kotlin 2.0.21.** Jewel's standalone repo is
> archived (read-only since April 2025); development moved into
> `intellij-community` (`platform/jewel`). Jewel **0.35 (2026-03-30)** and
> **0.36 (2026-04-27)** both require **CMP 1.10.0**. And CMP ≥ 1.8.0 requires
> **Kotlin ≥ 2.1.0**. So Jewel cannot coexist with the current toolchain.

The owner's binding decision (already made — not open for re-litigation): **on
this throwaway branch, upgrade the toolchain and use real Jewel.** Do *not* fall
back to a custom theme unless Jewel cannot run *even after* the upgrade. Because
the branch is never merged, the upgrade's blast radius is exactly zero.

This section proves the *chosen path* — upgraded toolchain + real Jewel —
actually resolves and renders **before any screen work begins**. If the
toolchain does not resolve, the Material baseline breaks, or Jewel cannot draw a
window, the experiment is dead here and we either fix it or invoke the documented
last-resort fallback (embedded below). Nothing in sections 02–06 starts until
this gate passes.

**API-shape note.** Section §2 of `claude-plan.md` contains Kotlin showing a
`PrototypeRoot` / `DecoratedWindow` / `TitleBar` structure. That Kotlin is
**illustrative pseudocode only**. The real Jewel `DecoratedWindow` /
`TitleBar` API has a specific `DecoratedWindowScope` / slot contract that
**differs by Jewel version**. Any Jewel code written in this section (the smoke
file) MUST follow the **pinned Jewel version's official sample structure**, not
the plan's pseudocode.

## Requirements

1. Create branch `proto/jewel-look-feel` off `feat/compose-desktop-rewrite`.
   **Local only — never push.** `git status` clean. Prove no other branch
   received commits.
2. Pin the Gradle wrapper up front (decide now, not reactively) to a version
   that supports Kotlin 2.1.x.
3. Upgrade the version catalog: CMP 1.7.3 → 1.10.0, Kotlin 2.0.21 → 2.1.x,
   compose-compiler plugin = Kotlin version, Decompose 3.2.2 → 3.3.x,
   coroutines 1.8.1 → 1.9.0; add Jewel version + library aliases.
4. Add the `kpm/public` Maven repo to `settings.gradle.kts` (keep existing
   repos).
5. Add Jewel dependencies to `desktopApp/build.gradle.kts` `desktopMain`.
6. **GATE-CRITICAL:** verify `compose.materialIconsExtended` still resolves on
   CMP 1.10 — the existing Material screens are the comparison baseline; if
   this breaks, the whole experiment is void.
7. Resolve the Decompose ↔ Compose version skew: force/align the Compose
   version Decompose's `extensions-compose` pulls to CMP 1.10.
8. Resolve the build (`:desktopApp:dependencies` then a clean
   `:desktopApp:compileKotlinDesktop`); fix version skew iteratively. Under
   **D7**, a *non-target* screen that blocks compile may be bumped or stubbed
   — **never** the two target screens or VM/domain logic.
9. Write a throwaway `PrototypeSmoke.kt` using the pinned Jewel version's
   **official sample structure**: a `DecoratedWindow` + `TitleBar` + Jewel
   theme + Jewel button + a Russian-text label.
10. **Gate decision:** owner runs the smoke; if it renders (Jewel window +
    Jewel button + Cyrillic without tofu) → delete the smoke file, proceed to
    section 02. If Jewel cannot run even after a reasonable upgrade effort
    (the only true blocker) → take the documented last-resort §B4 fallback
    (embedded below) and record it in the README.

## Dependencies

- **Requires:** none — this is the first section (hard gate).
- **Blocks:** 02, 03, 04, 05, 06. Nothing else may start until this gate
  passes (toolchain resolves, Material baseline intact, Jewel smoke renders
  OR §B4 fallback recorded).

## Implementation Details

### Step 1 — Branch (local only, prove isolation)

```bash
cd /Users/dmitrymedvedev/projects/pets/meeting-assistant
git checkout feat/compose-desktop-rewrite
git rev-parse HEAD                      # record base SHA, call it $BASE
git checkout -b proto/jewel-look-feel
git status                             # must be clean
```

- **Do NOT push.** The throwaway branch stays local; the commits *are* the
  deliverable. Push only if the owner explicitly asks.
- Prove other branches are untouched after all work in this section:
  - `git rev-parse feat/compose-desktop-rewrite` still equals `$BASE`.
  - `git rev-parse main` unchanged vs its pre-work SHA.
  - `git log --oneline feat/compose-desktop-rewrite..proto/jewel-look-feel`
    shows only commits created on this branch; the reverse range
    (`proto/jewel-look-feel..feat/compose-desktop-rewrite`) is empty.
  - `git status` clean at section end.

### Step 2 — Pin the Gradle wrapper up front

Edit `ui-compose/gradle/wrapper/gradle-wrapper.properties`. Set the
`distributionUrl` to a Gradle version that supports Kotlin 2.1.x — use
**8.13** (current wrapper is 8.10; decide this now, do not discover it's too
old mid-build):

```properties
distributionUrl=https\://services.gradle.org/distributions/gradle-8.13-bin.zip
```

If during build resolution a higher wrapper turns out to be required, bump it
and record the final number in the README — but commit to a concrete version
here rather than reacting later.

### Step 3 — Version catalog upgrade

Edit `ui-compose/gradle/libs.versions.toml`. Apply the full version table
below. The implementer **must confirm each artifact resolves** during Step 8;
if a newer patch is required, bump it (the throwaway branch authorizes this)
and record the final numbers in the README.

**Full version pin table (transcribed inline — starting point, verify by
Gradle resolution):**

| Artifact | Coordinate | From | To (target) |
|---|---|---|---|
| Compose Multiplatform | `org.jetbrains.compose` | 1.7.3 | **1.10.0** |
| Kotlin | `org.jetbrains.kotlin.multiplatform` | 2.0.21 | **2.1.20** (any 2.1.x CMP 1.10 accepts) |
| Compose compiler plugin | `org.jetbrains.kotlin.plugin.compose` | 2.0.21 | **= Kotlin version** (2.1.20) |
| Jewel standalone | `org.jetbrains.jewel:jewel-int-ui-standalone` | — | **0.36-** line for CMP 1.10 (build matching IJP 2026.1.x) |
| Jewel decorated window | `org.jetbrains.jewel:jewel-int-ui-decorated-window` | — | same as Jewel standalone |
| Decompose | `com.arkivanov.decompose` | 3.2.2 | **3.3.x** (Kotlin 2.1-compatible) |
| Decompose extensions-compose | `com.arkivanov.decompose:extensions-compose` | 3.2.2 | same as Decompose |
| kotlinx-coroutines | `org.jetbrains.kotlinx:kotlinx-coroutines-*` | 1.8.1 | **1.9.0** (Kotlin 2.1-friendly) |
| markdown-renderer-m3 | `com.mikepenz:multiplatform-markdown-renderer-m3` | 0.30.0 | bump to a CMP-1.10 build **or** stub the consuming screen (D7) |
| Gradle wrapper | (wrapper) | 8.10 | **8.13** (bump higher only if Kotlin 2.1.x demands it) |
| jna | `net.java.dev.jna:jna` | 5.15.0 | unchanged (not on prototype path — no FFI) |

Concrete catalog edits:

- `[versions]`: set `kotlin = "2.1.20"`, `compose = "1.10.0"`,
  `decompose = "3.3.x"` (resolve exact patch), `coroutines = "1.9.0"`. Add
  `jewel = "0.36-<ijpBuild>"` (the published coordinate whose release notes
  state **CMP 1.10.0** — pick the build matching IJP 2026.1.x; if `0.36` is
  unavailable, use the nearest CMP-1.10 Jewel release and record which).
- Compose-compiler plugin pin (`org.jetbrains.kotlin.plugin.compose`) MUST
  equal the Kotlin version (2.1.20). Update the plugins section accordingly.
- `[libraries]`: add Jewel aliases:
  - `jewel-int-ui-standalone = { group = "org.jetbrains.jewel", name = "jewel-int-ui-standalone", version.ref = "jewel" }`
  - `jewel-int-ui-decorated-window = { group = "org.jetbrains.jewel", name = "jewel-int-ui-decorated-window", version.ref = "jewel" }`

> Jewel artifacts are suffixed by IntelliJ-platform build (e.g.
> `0.36-…`). Pick the published coordinate whose release notes state CMP
> 1.10.0. Record the exact resolved coordinate in the README.

### Step 4 — Add the kpm/public Maven repo

Edit `ui-compose/settings.gradle.kts`. Add the JetBrains kpm/public Maven repo
to `dependencyResolutionManagement` (and to `pluginManagement` **if** plugin
resolution needs it). **Keep all existing repos** (`gradlePluginPortal`,
`mavenCentral`, `google`,
`maven("https://maven.pkg.jetbrains.space/public/p/compose/dev")`):

```kotlin
maven("https://packages.jetbrains.team/maven/p/kpm/public/")
// keep existing: gradlePluginPortal(), mavenCentral(), google(),
//   maven("https://maven.pkg.jetbrains.space/public/p/compose/dev")
```

### Step 5 — Add Jewel deps to desktopApp

Edit `ui-compose/desktopApp/build.gradle.kts`. Add to the `desktopMain`
source-set dependencies:

```kotlin
implementation(libs.jewel.int.ui.standalone)
implementation(libs.jewel.int.ui.decorated.window)
```

### Step 6 — Material-baseline integrity check (GATE-CRITICAL)

The existing Material screens (`AppContent`, `Sidebar`, `SettingsScreen`) are
the **comparison baseline**. They pervasively use Material icons via
`compose.materialIconsExtended` (declared in `shared/build.gradle.kts`
commonMain). That artifact has been **repackaged/deprecated across CMP
releases**. If it does **not** resolve on CMP 1.10, the Material screens fail
to compile, the comparison itself is impossible, and **the entire experiment is
void** — this is a Phase-0 failure, not a Phase-1 surprise.

Verify and, if needed, repair here:

- Confirm `compose.materialIconsExtended` resolves under CMP 1.10 via the
  dependency resolution in Step 8.
- If it does **not** resolve: fix it in this section by either
  - adding the standalone `org.jetbrains.compose.material:material-icons-extended`
    coordinate (the CMP 1.10-compatible replacement), **or**
  - replacing the specific extended-icon usages in the Material screens with
    core `material-icons-core` equivalents.
- A broken Material baseline blocks the gate. Do not proceed to the smoke
  until the Material screens compile.

### Step 7 — Decompose ↔ Compose alignment

Decompose `extensions-compose` transitively pulls a Compose version. If that
conflicts with CMP 1.10, it produces Skiko/runtime skew (opaque crashes,
compose-compiler/metadata conflicts).

- Inspect the dependency tree (Step 8 command) for the Compose version
  Decompose's `extensions-compose` pulls.
- If it conflicts with CMP 1.10, **force/align it to 1.10**. Forcing is
  acceptable here (branch is throwaway). Example in
  `desktopApp/build.gradle.kts`:

```kotlin
configurations.all {
    resolutionStrategy.eachDependency {
        if (requested.group == "org.jetbrains.compose" ||
            requested.group.startsWith("org.jetbrains.compose.")) {
            useVersion("1.10.0")
        }
    }
}
```

Tune the predicate to whatever the actual tree shows. Record any forced
versions in the README.

### Step 8 — Resolve the build

```bash
cd ui-compose
./gradlew :desktopApp:dependencies --configuration desktopRuntimeClasspath
./gradlew clean :desktopApp:compileKotlinDesktop
```

Fix version skew **iteratively**: align `org.jetbrains.compose` vs any
`androidx.compose` artifacts so a single coherent Compose generation wins.
Repeat the two commands until `compileKotlinDesktop` succeeds.

**Under D7 (non-target screen blocking compile):** the most likely offender is
the **markdown-renderer Protocol-detail screen** (uses
`multiplatform-markdown-renderer-m3` 0.30.0, which may have no CMP-1.10 build).
If a non-target screen blocks compilation:

- **Preferred:** bump `markdown-renderer-m3` to a CMP-1.10-compatible build.
- **If no compatible build exists:** replace that screen's body with a
  `// PROTOTYPE STUB` placeholder composable (keep the function signature so
  navigation still resolves), and note the stubbed screen in the README.

**NEVER** alter the two target screens (`Sidebar`/MeetingList,
`SettingsScreen`), `AppContent`, or any VM/domain logic. Only *non-target*
screens may be stubbed, and only if they block the upgrade.

New compiler warnings inherent to the CMP/Kotlin upgrade are acceptable on
this throwaway branch (note them in the README). Do not introduce *new* logic
warnings in the Material paths.

### Step 9 — Throwaway Jewel smoke file

Create `ui-compose/desktopApp/src/desktopMain/kotlin/PrototypeSmoke.kt`. It is
a `fun main() = application { ... }` that renders, **following the pinned Jewel
version's official `DecoratedWindow` sample structure** (do NOT copy the
pseudocode from `claude-plan.md` §2 — its `DecoratedWindow`/`TitleBar` shape is
not asserted API-correct), the following:

- A Jewel `DecoratedWindow` with the version-correct scope/slot usage.
- A `TitleBar` inside it (per the sample's `DecoratedWindowScope` contract).
- A Jewel theme wrapper (`IntUiTheme` dark, from
  `org.jetbrains.jewel.intui.standalone.theme.*`).
- A Jewel button (e.g. `DefaultButton`).
- A **Russian-text label** rendered with Jewel's text component, e.g.
  `Text("Встречи — проверка кириллицы")` — this is the Cyrillic glyph check.

Resolve the exact import paths and slot signatures from the pinned Jewel
version's documented standalone `DecoratedWindow` sample. The goal is the
minimum that proves: Jewel composes, the decorated window appears, a
Jewel-styled button draws, and Cyrillic renders **without tofu** (no
boxes/missing glyphs). If Jewel's default Int UI font shows tofu for Cyrillic,
supply a Cyrillic-capable `FontFamily` to the Jewel `TextStyle` /
`ThemeDefinition` in the smoke (note this, as section 02/03 will need the same
fix).

Temporarily point the Compose Desktop `application.mainClass` in
`desktopApp/build.gradle.kts` at `PrototypeSmokeKt` for the run.

**The assistant cannot launch a GUI.** Deliver the branch in a compiling
state with the exact command, and the owner performs the visual run:

```bash
cd ui-compose
# application.mainClass temporarily = "PrototypeSmokeKt"
./gradlew :desktopApp:run
```

The owner confirms: a Jewel window appears, with a Jewel-styled button, and
the Russian label renders **without tofu**.

### Step 10 — Gate decision

- **Renders correctly** (Jewel window + Jewel button + correct Cyrillic):
  delete `PrototypeSmoke.kt`, revert the temporary `mainClass` change, commit
  the toolchain upgrade, and **proceed to section 02**.
- **Jewel cannot run even after a reasonable upgrade effort** (this is the
  *only* true blocker — toolchain not resolving / Material baseline
  unrepairable / Jewel refuses to compose/render): take the **documented
  last-resort fallback** below. This is **not a cliff** — the recipe is
  concrete and actionable. Record the fallback decision **prominently in the
  README**. The verdict is still produced (the comparison runs against a
  custom compact non-Material theme on the upgraded CMP instead of Jewel).

#### Last-resort fallback recipe (claude-research.md §B4 — embedded inline)

Only if Jewel is a true blocker. Implement the same two screens
(MeetingList/Sidebar, Settings) with a **custom compact non-Material theme on
the upgraded CMP** (no Jewel), still toggled against the unmodified Material
`AppContent`. The "mobile feel" is largely density/spacing/typography and is
addressable without Jewel.

**Density token targets (Material 3 default → desktop-native):**

| Aspect | Material 3 default | Desktop-native target |
|---|---|---|
| Button height | ~48.dp | **~28–32.dp** |
| Inter-element spacing | ~16.dp | **~6–8.dp** |
| Body/UI text size | ~16.sp | **~13–14.sp** |
| Checkbox/radio | 24×24 | 16×16 |
| Hover | none | explicit hover bg/affordance |

**Build a minimal non-Material theme as:**

1. A **token object** exposing:
   - `colors` — dark scheme reusing the app's existing palette
     (dark primary `0xFF4FA3E0`, surfaces, onSurface) so the comparison is
     fair.
   - `spacing` — compact values: ~6–8.dp inter-element gaps.
   - `typography` — ~13–14.sp body/UI text; a **system font**
     (SF Pro / Segoe UI / Inter — bundle Inter/Noto if needed for Cyrillic).
   - `dimensions` — ~28–32.dp control heights.
2. A handful of **compact composables** (enough to re-skin both screens):
   - **Button** — slim fixed-height (~30.dp), explicit hover background via
     `Modifier.hoverable` / pointer-hover state.
   - **Text field** — compact single-line, slim padding.
   - **List row** — tight `contentPadding`, explicit hover/selection
     background.
   - **Section header** — small uppercase/dense label for Settings sections.
3. Explicit **hover states** everywhere interactive
   (`Modifier.hoverable` + a hover background; Material has none by default).
4. System font via the typography token; ensure Cyrillic renders (bundle a
   Cyrillic-capable family if the system font is unavailable).

This re-skins the same two screens with desktop density and preserves the
experiment's core question (theming vs stack limit). Record in the README that
the §B4 fallback was taken and that the comparison is custom-theme-vs-Material
(not Jewel-vs-Material).

## Acceptance Criteria

- [ ] `proto/jewel-look-feel` exists, branched off
      `feat/compose-desktop-rewrite`; **local only — not pushed** (unless the
      owner explicitly asked).
- [ ] Other branches **provably untouched**: `feat/compose-desktop-rewrite`
      and `main` HEAD SHAs unchanged vs pre-work;
      `git log feat/compose-desktop-rewrite..proto/jewel-look-feel` shows only
      this branch's commits; reverse range empty; `git status` clean.
- [ ] Gradle wrapper pinned in
      `ui-compose/gradle/wrapper/gradle-wrapper.properties` to a Kotlin-2.1.x-
      capable version (8.13, or the recorded higher version actually required).
- [ ] `libs.versions.toml` upgraded: CMP 1.10.0, Kotlin 2.1.x,
      compose-compiler plugin = Kotlin version, Decompose 3.3.x, coroutines
      1.9.0; Jewel version + `jewel-int-ui-standalone` /
      `jewel-int-ui-decorated-window` library aliases added.
- [ ] `settings.gradle.kts` has the
      `https://packages.jetbrains.team/maven/p/kpm/public/` repo; all existing
      repos retained.
- [ ] Jewel deps added to `desktopApp/build.gradle.kts` `desktopMain`.
- [ ] Upgraded toolchain **resolves**: `:desktopApp:dependencies` runs and a
      clean `:desktopApp:compileKotlinDesktop` succeeds.
- [ ] **`compose.materialIconsExtended` intact** on CMP 1.10 (Material
      baseline compiles); if repackaged, the standalone coordinate /
      icon-usage fix is applied and recorded.
- [ ] Decompose's transitive Compose version aligned/forced to CMP 1.10 (no
      `org.jetbrains.compose` vs `androidx.compose` skew in the resolved
      classpath).
- [ ] Two target screens (MeetingList/Sidebar, Settings), `AppContent`, and
      all VM/domain logic **unmodified**; any non-target screen stubbed under
      D7 is noted for the README.
- [ ] `PrototypeSmoke.kt` written using the **pinned Jewel version's official
      sample structure** (not the plan's pseudocode): DecoratedWindow +
      TitleBar + Jewel theme + Jewel button + Russian label.
- [ ] **Gate outcome recorded:** either the owner confirms the smoke renders
      (Jewel window + Jewel button + **Cyrillic correct, no tofu**), the smoke
      file is deleted, and section 02 is unblocked — **OR** the §B4
      custom-compact-theme last-resort fallback decision is recorded in the
      README and the path forward documented.

## Files to Create/Modify

**Create:**

- `/Users/dmitrymedvedev/projects/pets/meeting-assistant/ui-compose/desktopApp/src/desktopMain/kotlin/PrototypeSmoke.kt`
  — throwaway Jewel smoke (`fun main() = application { ... }`); **deleted in
  Step 10 if the gate passes.**

**Modify:**

- `/Users/dmitrymedvedev/projects/pets/meeting-assistant/ui-compose/gradle/wrapper/gradle-wrapper.properties`
  — pin Gradle 8.13 (Step 2).
- `/Users/dmitrymedvedev/projects/pets/meeting-assistant/ui-compose/gradle/libs.versions.toml`
  — version upgrades + Jewel aliases (Step 3).
- `/Users/dmitrymedvedev/projects/pets/meeting-assistant/ui-compose/settings.gradle.kts`
  — add kpm/public Maven repo (Step 4).
- `/Users/dmitrymedvedev/projects/pets/meeting-assistant/ui-compose/desktopApp/build.gradle.kts`
  — Jewel deps + Decompose/Compose resolution-strategy alignment +
  temporary `application.mainClass = "PrototypeSmokeKt"` for the smoke run
  (reverted in Step 10) (Steps 5, 7, 9).

**Conditionally modify (only if forced by the upgrade — D7, noted for README):**

- `/Users/dmitrymedvedev/projects/pets/meeting-assistant/ui-compose/shared/build.gradle.kts`
  — only if `compose.materialIconsExtended` must be replaced with the
  standalone `material-icons-extended` coordinate (Step 6).
- A **non-target** screen file (most likely the markdown-renderer
  Protocol-detail screen under
  `/Users/dmitrymedvedev/projects/pets/meeting-assistant/ui-compose/shared/src/commonMain/kotlin/ui/screens/`)
  — `// PROTOTYPE STUB` body swap only if it blocks compile and no
  CMP-1.10 markdown-renderer build exists (Step 8). **Never** the two target
  screens, `AppContent`, or VM/domain logic.

**Deliverable:** branch with an upgraded, resolving build + a rendered Jewel
smoke window confirmed by the owner (or a recorded §B4 last-resort decision).
This is the hard gate; sections 02–06 do not start until it passes.
