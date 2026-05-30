# Ralph Loop: Desktop Look-and-Feel (Jewel) Prototype

## Mission

Implement a **throwaway, never-merged** prototype on branch
`proto/jewel-look-feel` (off `feat/compose-desktop-rewrite`) that re-renders
the Meeting Assistant app's **MeetingList** and **Settings** screens with
**JetBrains Jewel** on an upgraded **Compose Multiplatform 1.10 / Kotlin
2.1.x** toolchain, toggled at runtime against the **unmodified existing
Material 3** screens, running on **fake data with no FFI**, so the project
owner can render a per-screen "desktop vs mobile" verdict and close the
Qt-vs-Compose stack question.

Repo root: `/Users/dmitrymedvedev/projects/pets/meeting-assistant`.
All work is in `ui-compose/` (Kotlin Multiplatform Compose Desktop).

## Global execution rules

1. Implement sections **strictly in dependency order**:
   `01 → 02 → (03 ∥ 04) → 05 → 06` (see the embedded index below).
2. **Section 01 is a HARD GATE.** Do not begin any other section until it
   passes: the upgraded toolchain resolves, the Material baseline
   (`compose.materialIconsExtended`) is intact, and the Jewel smoke window
   renders with correct Cyrillic — OR the §B4 custom-compact-theme last-resort
   fallback is recorded. The §B4 recipe is embedded inside section 01.
3. For each section: read its embedded content, derive a TODO list from its
   **Acceptance Criteria**, implement, then verify **every** criterion before
   moving on.
4. **Isolation (hard rule):** never edit `shared` ViewModels/domain or the
   Rust core, except the sanctioned Gradle / version-catalog / run-config
   changes and — only if the upgrade forces it — a `// PROTOTYPE STUB` of a
   *non-target* screen (never the two target screens, `AppContent`, or
   VM/domain logic). The Material comparison **is** the unmodified
   `AppContent(root)`.
5. **Verify before writing code:** fakes and `PrototypeMain` are written
   against the *actual* `RootComponent`/repository signatures and `Main.kt`'s
   actual Decompose lifecycle wiring (read them first), never against
   illustrative pseudocode. Failing this causes blank screens.
6. **Jewel API shape** differs by version — always follow the pinned Jewel
   version's official samples, not snippets in these documents.
7. The branch stays **local and unmerged** — never push unless the owner
   explicitly asks.
8. Sections 01 (smoke run) and 06 (verdict) require the **owner** to run the
   desktop GUI (the agent cannot launch one). At those points, deliver a
   compiling branch + the exact documented commands and stop for owner
   action as the section instructs.

## How section completion works

Process sections in order. A section is complete only when every checkbox in
its **Acceptance Criteria** is satisfied. When **all six** sections are
complete and verified, emit the completion signal on its own line:

<promise>ALL-SECTIONS-COMPLETE</promise>

Do not emit the promise early. Sections 01 and 06 may require pausing for the
owner's GUI run — treat the owner's confirmation/recorded verdict as the
criterion's evidence.

---

# EMBEDDED: sections/index.md

<!-- SECTION_MANIFEST
section-01-toolchain-jewel-gate
section-02-fakes-dev-entry-material
section-03-jewel-meeting-list
section-04-jewel-settings
section-05-toggle-window-readme
section-06-verdict-closeout
END_MANIFEST -->

# Implementation Sections Index — Desktop Look-and-Feel (Jewel) Prototype

Throwaway, never-merged prototype that re-renders MeetingList + Settings with
JetBrains Jewel on an upgraded toolchain, toggled at runtime against the
existing Material 3 screens, so the owner can decide "desktop vs mobile" and
close the Qt-vs-Compose question. Source plan: `../claude-plan.md`.

## Dependency Graph

| Section | Depends On | Blocks | Parallelizable |
|---------|------------|--------|----------------|
| section-01-toolchain-jewel-gate | - | 02, 03, 04, 05, 06 | No (hard gate) |
| section-02-fakes-dev-entry-material | 01 | 03, 04, 05 | No |
| section-03-jewel-meeting-list | 02 | 05 | Yes (with 04) |
| section-04-jewel-settings | 02 | 05 | Yes (with 03) |
| section-05-toggle-window-readme | 03, 04 | 06 | No |
| section-06-verdict-closeout | 05 | - | No (owner-driven) |

## Execution Order

1. **section-01-toolchain-jewel-gate** — HARD GATE. Nothing else starts until
   the upgraded toolchain resolves, the Material baseline survives, and Jewel
   renders a smoke window (or the §B4 fallback is recorded).
2. **section-02-fakes-dev-entry-material** — after 01. Fakes + dev entry +
   lifecycle; Material variant runs on fake data with no FFI.
3. **section-03-jewel-meeting-list** and **section-04-jewel-settings** — may
   run in parallel after 02.
4. **section-05-toggle-window-readme** — after both 03 and 04.
5. **section-06-verdict-closeout** — owner builds/runs/records; terminal.

## Section Summaries

### section-01-toolchain-jewel-gate
Phase 0 hard gate. Create `proto/jewel-look-feel` off
`feat/compose-desktop-rewrite` (local only, never pushed). Pin Gradle wrapper.
Upgrade CMP 1.7.3→1.10.0, Kotlin 2.0.21→2.1.x, compose-compiler, Decompose,
coroutines; add Jewel deps + kpm/public repo. Gate-critical: verify
`compose.materialIconsExtended` still resolves (else Material baseline — the
comparison itself — breaks); align Decompose's transitive Compose version.
Render a Jewel smoke window (DecoratedWindow + TitleBar + button + Cyrillic
label). Decision point: Jewel renders → continue; cannot run → documented
last-resort custom compact non-Material theme per `claude-research.md` §B4.

### section-02-fakes-dev-entry-material
Phase 1. **Read first**: actual `RootComponent`/repository signatures and
`Main.kt`'s exact Decompose lifecycle wiring. Build fake
Meeting/Settings/Recording/Diagnostics repositories + `SampleData`
(populated/empty/loading/error fixtures). `PrototypeMain.kt` (no `initCore`,
no dylib, lifecycle replicated verbatim) + `UiVariant` + `PrototypeRoot`
rendering the **unmodified** `AppContent(root)` for Material. Run path =
`application.mainClass = PrototypeMainKt` on the branch. Exit: Material
MeetingList + Settings render from fakes on a bare JDK; navigation works.

### section-03-jewel-meeting-list
Phase 2. `JewelTheme.kt` (dark `IntUiTheme`, no nested MaterialTheme,
Cyrillic-capable font verified). `JewelAppShell.kt` mirroring `AppContent`'s
sidebar|divider|content `Row` and `root.screen` switching.
`JewelMeetingListScreen.kt` mirroring the Sidebar (header + add/refresh,
LazyColumn rows with name/date/status chips, selection/hover, empty/loading/
error, footer nav) using Jewel components and the same VM/nav callbacks.

### section-04-jewel-settings
Phase 3. `JewelSettingsScreen.kt` mirroring every Settings control
section-for-section in Jewel (API key secret field + show/hide + validation;
recording source segmented + echo toggle; protocol template dropdown;
language segmented + accuracy slider 1–5 + threads slider 0–16; storage path
fields with `/`/`~` validation + model dropdown + custom path + prompts dir;
Save button enabled-rule + feedback; toolbar + back). Verbatim Russian labels.
Loads fixtures via the same repository calls as the Material screen.

### section-05-toggle-window-readme
Phase 4. `VariantToggleBar`: always-visible `{Material|Jewel}` + data-state
`{Populated·Empty·Loading·Error}` controls (or documented compile-time
fixture flag if the picker is dropped). Wire `USE_DECORATED_WINDOW`: Jewel →
DecoratedWindow+TitleBar, Material → standard Window; macOS fallback keeps
Jewel content in a standard Window. Default = Jewel on launch; live flip, no
JVM restart, nav works in both. Write `ui-compose/PROTOTYPE.md` (purpose +
never-merge warning, exact build/run commands, final resolved version pins
incl. exact Jewel coordinate, every dep bumped, any stubbed screen, macOS
fallback, fallback-taken note, verdict template).

### section-06-verdict-closeout
Phase 5, owner-driven. Owner builds, runs, toggles both screens, fills the
per-screen verdict template in `PROTOTYPE.md`. Record outcome — (a) theming
fixes it → stay on Compose, open real restyle work, drop Qt; or (b) still not
desktop → open Qt-migration plan. Update project memory (Qt-vs-Compose
question + `project_goal_shipping`) with the dated decision. Branch left
unmerged and local.


---

# EMBEDDED: sections/section-01-toolchain-jewel-gate.md

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


---

# EMBEDDED: sections/section-02-fakes-dev-entry-material.md

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


---

# EMBEDDED: sections/section-03-jewel-meeting-list.md

# Section 03: Jewel MeetingList Screen + Jewel Theme/Shell

## Background

**Meeting Assistant** is a Rust + Kotlin Multiplatform Compose Desktop app
(`/ui-compose`). This is a **throwaway, never-merged** prototype to decide
whether the app's "mobile feel" is a theming choice (fixable) or a stack
limitation (justifies Qt). Sections 01–02 upgraded the toolchain to
CMP 1.10 / Kotlin 2.1.x with real Jewel, and stood up a dev entry that runs
the **existing Material screens** on fake data with no FFI.

This section builds the **Jewel rendering of the MeetingList screen**, plus
the shared Jewel theme and app shell that section 04's Settings screen also
uses. Fidelity rule (**D6**): mirror **content & density, not pixels** — same
data, same controls, same sections, but desktop-idiomatic sizing/spacing
(Jewel's Int UI gives ~28–32.dp controls, ~6–8.dp spacing, ~13–14.sp text for
free). All labels are Russian and copied **verbatim** from the Material
screens.

### What the Material MeetingList looks like (the thing to mirror)

The "MeetingList" is really the **Sidebar**
(`ui-compose/shared/src/commonMain/kotlin/ui/Sidebar.kt`, ~lines 24–93):

- A header with title **"Встречи"** and add (`Icons.Default.Add`) + refresh
  `IconButton`s.
- A `LazyColumn` of `MeetingListItem` rows. Each row shows: meeting **name**
  (body text), **`formatDate(createdAt)`** (small label), and conditional
  status chips **"Транскрипт"** (if `hasTranscript`) / **"Протокол"** (if
  `hasProtocol`). The selected row has a highlighted background.
- Empty state: centered **"Нет встреч"**.
- Footer: Settings + Diagnostics icon buttons.

State is `MeetingListState { Loading | Success(List<Meeting>) | Error(msg) }`
exposed by `root.meetingListViewModel.state`. `Meeting` =
`{ id, name, audioPath, hasTranscript, hasProtocol, createdAt: Long }`. The
overall app layout (`AppContent`) is a `Row`: sidebar (~280.dp) |
`VerticalDivider` | content pane that switches on the sealed `Screen` value
via Decompose `subscribeAsState()`.

## Requirements

When this section is complete:

- A dark Jewel theme wrapper exists, renders Cyrillic correctly, and does
  **not** nest a Material `MaterialTheme`.
- A `JewelAppShell` mirrors `AppContent`'s sidebar | divider | content layout
  and switches content on `root.screen`.
- `JewelMeetingListScreen` faithfully mirrors the Sidebar's content & density
  (header + add/refresh, list rows with name/date/status chips,
  selection/hover, empty/loading/error, footer nav) using Jewel components and
  the **same VM/nav callbacks** the Material Sidebar uses.
- It compiles and renders from the fake data set up in section 02.

## Dependencies

- **Requires:** section 02 (fakes, dev entry, `PrototypeRoot`, Material
  variant running). Section 01 gate passed.
- **Blocks:** section 05 (toggle/window/README needs both Jewel screens).
- **Parallelizable with:** section 04 (Jewel Settings) — both depend only on
  02, write different files.

## Implementation Details

> **API-shape caveat:** all Jewel APIs (`IntUiTheme`, theme providers,
> component names, `LazyColumn`/list, icon buttons) differ by Jewel version.
> Resolve exact names/signatures from the **pinned Jewel version's official
> standalone samples/docs**, not from memory or the plan's pseudocode. Use the
> closest Jewel primitive where an exact equivalent is missing and note it.

### Step 1 — `JewelTheme.kt` (shared by sections 03 & 04)

`ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelTheme.kt`:

- A composable that wraps content in Jewel's **dark** `IntUiTheme`
  (`org.jetbrains.jewel.intui.standalone.theme.*`). Match the app's dark
  default so the Material↔Jewel comparison is fair.
- **Do NOT** nest a Material `MaterialTheme` inside it — Jewel and Material
  must live in **separate composable subtrees** (nested Material values win
  and break Jewel styling). The Jewel screen set is wholly Jewel.
- Expose any shared spacing/typography tokens the two Jewel screens need.
- **Cyrillic check (Phase-2 exit requirement):** every label is Russian. Run
  the screen and confirm Jewel's default Int UI font renders Cyrillic. If it
  shows tofu/boxes, supply a Cyrillic-capable `FontFamily` (bundled
  Inter/Noto, or a system font) to the Jewel `TextStyle` / `ThemeDefinition`.
  A font bug here would invalidate the owner's verdict — treat correct
  Cyrillic as mandatory before this section is "done". (Section 04 reuses this
  theme; fixing it here fixes it for Settings too.)

### Step 2 — `JewelAppShell.kt`

`ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelAppShell.kt`:

- A `Row` mirroring `AppContent`'s layout: Jewel sidebar (~280.dp) | a
  divider | a content pane (weight 1).
- Subscribe to `root.screen` via Decompose `subscribeAsState()` exactly like
  `AppContent` does, and `when` over the sealed `Screen` value to render
  `JewelMeetingListScreen` / `JewelSettingsScreen` (section 04).
- For any other `Screen` value (detail, recording, diagnostics, etc.) render a
  simple Jewel placeholder ("Not in prototype scope") so navigation never
  crashes.
- The sidebar pane hosts `JewelMeetingListScreen` (the list *is* the
  sidebar), matching the Material structure.

### Step 3 — `JewelMeetingListScreen.kt`

`ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelMeetingListScreen.kt`:

- Consume `root.meetingListViewModel.state` (the same `MeetingListState`
  StateFlow the Material Sidebar uses — collect as Compose state).
- **Header row:** title **"Встречи"** + Jewel icon buttons for **add** and
  **refresh**, wired to the **same callbacks** the Material Sidebar uses
  (the add → new-meeting action; refresh → `meetingListViewModel.refresh()`
  or whichever method the real Sidebar calls — confirm from `Sidebar.kt`).
- **`Success`:** a Jewel `LazyColumn` of rows. Each row: meeting **name**,
  **`formatDate(createdAt)`** (reuse the existing `formatDate` helper from
  `shared` — do not reimplement), conditional **"Транскрипт"** /
  **"Протокол"** status chips. Selected/hovered row gets Jewel
  selection/hover styling. Click → `root.onMeetingSelected(meeting)` (exact
  nav method per `RootComponent.kt`).
- **`Loading`:** Jewel progress/spinner. **`Error`:** the error message.
- **Empty (`Success` with empty list):** centered **"Нет встреч"**.
- **Footer:** Jewel icon buttons → `root.onSettingsRequested()` (and the
  diagnostics nav method, matching the Material footer).
- Density: compact desktop sizing — let Jewel Int UI defaults drive it; don't
  re-inflate to Material touch sizes.

Reuse `shared` helpers (`formatDate`, domain types) by import — **do not edit
`shared`**. Use only the public VM/nav surface of `RootComponent`.

### Step 4 — Wire into `PrototypeRoot`

Replace the section-02 Jewel placeholder so the Jewel branch of
`PrototypeRoot` renders `JewelTheme { JewelAppShell(root) }`. (The full
toggle/decorated-window wiring is section 05; here just make the Jewel branch
show real Jewel content for verification.)

## Acceptance Criteria

- [ ] `JewelTheme` wraps content in Jewel's **dark** `IntUiTheme`; no nested
      Material `MaterialTheme`.
- [ ] **Cyrillic renders correctly** in the Jewel variant (no tofu) — a
      Cyrillic-capable font is supplied if Jewel's default lacks glyphs.
- [ ] `JewelAppShell` mirrors `AppContent`'s sidebar|divider|content layout
      and switches on `root.screen` via Decompose `subscribeAsState()`; other
      screens show a safe placeholder (no crash on navigation).
- [ ] `JewelMeetingListScreen` mirrors the Sidebar's content & density:
      "Встречи" header + add/refresh, list rows with name +
      `formatDate(createdAt)` + "Транскрипт"/"Протокол" chips +
      selection/hover, "Нет встреч" empty state, Loading + Error states,
      footer nav.
- [ ] All actions use the **same VM/nav callbacks** as the Material Sidebar
      (`onMeetingSelected`, `onSettingsRequested`, refresh, …); `formatDate`
      and domain types reused from `shared` by import.
- [ ] Compiles; renders from the section-02 fakes; the Jewel branch of
      `PrototypeRoot` shows `JewelTheme { JewelAppShell(root) }`.
- [ ] No edits to `shared` ViewModels/domain or the Rust core; code isolated
      under `prototype/jewel/`.

## Files to Create/Modify

**Create:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelTheme.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelAppShell.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelMeetingListScreen.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelComponents.kt`
  (optional — small shared helpers like a section header / status chip)

**Modify:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/PrototypeRoot.kt` —
  Jewel branch renders `JewelTheme { JewelAppShell(root) }` instead of the
  placeholder.

**Do not modify:** `shared` (including `Sidebar.kt`, `AppContent.kt`,
ViewModels, domain), the Rust core.


---

# EMBEDDED: sections/section-04-jewel-settings.md

# Section 04: Jewel Settings Screen (Form-Density Stress Point)

## Background

**Meeting Assistant** is a Rust + Kotlin Multiplatform Compose Desktop app
(`/ui-compose`). This **throwaway, never-merged** prototype re-renders two
screens with JetBrains Jewel and toggles them against the existing Material 3
screens so the owner can decide "desktop vs mobile". Sections 01–03 upgraded
the toolchain (CMP 1.10 / Kotlin 2.1.x + Jewel), built fakes + dev entry, and
delivered the shared `JewelTheme`/`JewelAppShell` + `JewelMeetingListScreen`.

This section builds the **Jewel rendering of the Settings screen** — the
**form-density stress point** and the hardest test of "desktop vs mobile",
because a dense form is exactly where Material 3 reads as mobile. Fidelity
rule (**D6**): mirror **content & density, not pixels** — every control the
Material screen has, with desktop-idiomatic sizing. All labels are Russian,
copied **verbatim** from the Material screen.

### What the Material Settings screen has (mirror it section-for-section)

`ui-compose/shared/src/commonMain/kotlin/ui/screens/SettingsScreen.kt`
(~lines 27–429): a `Scaffold` + `SnackbarHost`, a `SettingsToolbar`
(TopAppBar, title **"Настройки"**, back arrow), and a `SettingsForm` with:

1. **Anthropic API** — `OutlinedTextField` labelled **"API Key"**, placeholder
   `sk-ant-…`, a password show/hide trailing icon, validation supporting text
   (error if non-blank and not starting with `sk-ant-`).
2. **Recording** — segmented control **"Источник звука"**
   {**Микрофон**, **Система**, **Оба**}; a switch **"Подавление эха"**.
3. **Protocol template** (conditional, only if templates exist) — dropdown:
   **"По умолчанию"** + each template name.
4. **Transcription** — language segmented {**Русский**, **English**, **Авто**};
   **"Точность распознавания"** slider 1–5 (3 steps) with explanatory text;
   **"CPU потоки"** slider 0–16 (15 steps), `0` shown as **"Авто"**.
5. **Storage** — two path fields (`OutlinedTextField`, placeholder
   `/home/user/…`, error if non-blank and not starting with `/` or `~`):
   meetings dir + db path; a **model** dropdown showing
   `"name  ·  sizeMb MB"` + description per model with a **"Другой путь…"**
   option that reveals a custom path field; a prompts-dir path field.
6. **Save** — a full-width button **"Сохранить"**,
   `enabled = pathsValid && apiKeyValid`, with snackbar feedback on save.

`Settings` = `{ paths{model,db,meetingsDir,prompts}, anthropicApiKey,
recording{source,echoCancel}, defaultTemplate,
transcriber{language,beamSize 1–5,nThreads 0=auto} }`. The screen loads
`settings.get()`, `templatesList()`, `modelsList()` in a `LaunchedEffect`
into local state.

## Requirements

When this section is complete:

- `JewelSettingsScreen` mirrors **every** Settings section/control above with
  Jewel components, verbatim Russian labels, the same validation rules and the
  same enabled-rule for Save.
- It loads fixtures via the **same repository calls** the Material screen uses
  (`settings.get()`, `templatesList()`, `modelsList()`), via the fakes.
- It compiles and renders fully from the section-02 fakes; rendered via the
  shared `JewelTheme` (Cyrillic correct).

## Dependencies

- **Requires:** section 02 (fakes/dev entry) and section 03 (`JewelTheme`,
  `JewelAppShell` — Settings is rendered inside the shell). Section 01 gate
  passed.
- **Blocks:** section 05 (toggle/window/README needs both Jewel screens).
- **Parallelizable with:** section 03 (different files) — but uses section
  03's `JewelTheme`/shell, so coordinate if run truly in parallel (the shell's
  content `when` must route the Settings `Screen` to `JewelSettingsScreen`).

## Implementation Details

> **API-shape caveat:** Jewel component names/signatures (`TextField`,
> radio/segmented, `Checkbox`/toggle, `Dropdown`/combo, sliders,
> `DefaultButton`/`OutlinedButton`, inline banners) differ by Jewel version.
> Resolve exact APIs from the **pinned Jewel version's official standalone
> samples/docs**. Where Jewel lacks an exact equivalent, use the closest
> Jewel primitive and note the substitution in `PROTOTYPE.md` (section 05).

### Step 1 — Load state (mirror the Material screen's shape)

In `JewelSettingsScreen`, in a `LaunchedEffect`, call the same repository
methods the Material screen calls — `root.settings.get()`,
`root.settings.templatesList()`, `root.settings.modelsList()` (exact names per
`SettingsRepository` / `RootComponent`) — and hold them in local Compose
state mirroring the Material screen's local-state shape so behavior matches.
Replicate the derived validation:

- `apiKeyValid` = api key blank, unchanged, or starts with `sk-ant-`.
- `pathsValid` = every path blank or starts with `/` or `~`.

### Step 2 — Sections, in order, verbatim Russian labels

Render with Jewel components, desktop density (let Jewel Int UI defaults set
sizing — compact, ~28–32.dp controls; do not inflate to Material touch sizes):

1. **Anthropic API:** Jewel text field labelled **"API Key"**, placeholder
   `sk-ant-…`, a secret/password mode with a show/hide affordance, and
   validation/supporting text (error styling if invalid).
2. **Recording:** **"Источник звука"** as a Jewel segmented/radio-chain
   {**Микрофон**, **Система**, **Оба**}; **"Подавление эха"** as a Jewel
   checkbox/toggle.
3. **Protocol template** (only if templates non-empty): a Jewel dropdown with
   **"По умолчанию"** + each template name.
4. **Transcription:** language Jewel segmented/radio {**Русский**,
   **English**, **Авто**}; **"Точность распознавания"** Jewel slider 1–5 with
   the explanatory text; **"CPU потоки"** Jewel slider 0–16, displaying
   **"Авто"** when value is 0.
5. **Storage:** two Jewel path fields (placeholder `/home/user/…`, error if
   non-blank & not `/`/`~`) for meetings dir + db; a Jewel **model** dropdown
   listing `"name  ·  sizeMb MB"` + description per model with a **"Другой
   путь…"** option revealing a custom path field; a prompts-dir path field.
6. **Save:** a Jewel button **"Сохранить"**,
   `enabled = pathsValid && apiKeyValid`; on click build the updated
   `Settings` and call the same save method the Material screen uses; show
   Jewel feedback (a Jewel inline status/banner — Jewel has no Material
   `Snackbar`; use the closest Jewel notification primitive or an inline
   status line, and note the substitution).

### Step 3 — Toolbar + routing

- A Jewel header **"Настройки"** with a back affordance →
  `root.onBackToList()` (exact nav method per `RootComponent.kt`).
- Ensure section 03's `JewelAppShell` content `when` routes the Settings
  `Screen` case to `JewelSettingsScreen` so navigation from the
  MeetingList footer ("Настройки") reaches it.

Reuse `shared` domain types by import. Use only the public repository/nav
surface. **Do not edit `shared`.**

## Acceptance Criteria

- [ ] `JewelSettingsScreen` loads `settings.get()` / `templatesList()` /
      `modelsList()` (same calls as the Material screen) via the fakes in a
      `LaunchedEffect`; local-state shape mirrors the Material screen.
- [ ] All six sections present with **verbatim Russian labels**: API key
      (secret + show/hide + validation), recording source segmented + echo
      toggle, protocol template dropdown, language segmented + accuracy slider
      1–5 + threads slider 0–16 ("Авто" at 0), storage path fields with
      `/`/`~` validation + model dropdown (`"name · sizeMb MB"` + desc) +
      "Другой путь…" custom path + prompts dir, Save button.
- [ ] Save is `enabled = pathsValid && apiKeyValid` with the same validation
      rules; clicking it calls the same save method and shows Jewel feedback.
- [ ] Toolbar "Настройки" + back → `root.onBackToList()`; navigation from the
      MeetingList footer reaches this screen via `JewelAppShell`.
- [ ] Renders inside the shared `JewelTheme`; **Cyrillic correct** (no tofu).
- [ ] Any Jewel-vs-Material control substitution is noted for `PROTOTYPE.md`.
- [ ] Compiles and renders fully from section-02 fakes; no edits to `shared`
      or the Rust core; code isolated under `prototype/jewel/`.

## Files to Create/Modify

**Create:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelSettingsScreen.kt`

**Modify:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelAppShell.kt`
  — route the Settings `Screen` case to `JewelSettingsScreen`.
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelComponents.kt`
  — only if shared form helpers (section header, labelled field) are factored
  out.

**Do not modify:** `shared` (including `SettingsScreen.kt`, ViewModels,
domain), the Rust core.


---

# EMBEDDED: sections/section-05-toggle-window-readme.md

# Section 05: Runtime Toggle + Decorated Window + macOS Fallback + README

## Background

**Meeting Assistant** is a Rust + Kotlin Multiplatform Compose Desktop app
(`/ui-compose`). This **throwaway, never-merged** prototype re-renders
MeetingList + Settings with JetBrains Jewel and toggles them against the
existing Material 3 screens so the owner can render a "desktop vs mobile"
verdict and close the Qt-vs-Compose stack question. Sections 01–04 upgraded
the toolchain (CMP 1.10 / Kotlin 2.1.x + Jewel), built fakes + dev entry +
the Material baseline, and delivered both Jewel screens inside a shared
`JewelTheme`/`JewelAppShell`.

This section makes it a **usable side-by-side comparison build**: a visible
in-window toggle, the Jewel decorated window with macOS fallback, and the
handoff `PROTOTYPE.md`. Binding decisions in force:

- **D4** Jewel variant uses Jewel's **`DecoratedWindow` + `TitleBar`**; the
  Material variant uses the standard `Window`. A macOS fallback (keeping
  Jewel *content* in a standard `Window`) sits behind a constant.
- **D5** Visible **in-window** toggle; **default = Jewel on launch**; flips
  live with **no JVM restart**; MeetingList↔Settings navigation works in both
  variants.

## Requirements

When this section is complete:

- A `VariantToggleBar` is always visible: a `{Material | Jewel}` control plus
  a `{Populated · Empty · Loading · Error}` data-state control (or a
  documented compile-time fixture flag if the picker proves fiddly).
- The window is chosen by variant: Jewel → `DecoratedWindow` + `TitleBar`;
  Material → standard `Window`; `USE_DECORATED_WINDOW = false` keeps Jewel
  content in a standard `Window` (macOS fallback).
- Launch default is **Jewel**; flipping the toggle changes variant live
  (in-process window re-create is acceptable; **no JVM restart**); navigation
  works in both variants.
- `ui-compose/PROTOTYPE.md` is written with purpose, exact build/run
  commands, final resolved version pins, every dependency bumped, any stubbed
  screen, the macOS fallback, whether the §B4 fallback was taken, and the
  verdict template.

## Dependencies

- **Requires:** sections 03 **and** 04 (both Jewel screens) — and 01/02.
- **Blocks:** section 06 (owner verdict & close-out).

## Implementation Details

> **API-shape caveat:** Jewel `DecoratedWindow` / `TitleBar` have a specific
> `DecoratedWindowScope` / slot contract that differs by Jewel version. Use
> the **pinned Jewel version's official decorated-window sample structure** —
> the `Column { TitleBar(); toggle(); shell() }` shape in `claude-plan.md` §2
> is illustrative pseudocode, not asserted API-correct.

### Step 1 — `VariantToggleBar`

In `prototype/PrototypeRoot.kt` (or a small `prototype/VariantToggleBar.kt`):

- An always-visible strip with **two** controls:
  - `{Material | Jewel}` segmented/toggle → sets the `variant` state.
  - `{Populated · Empty · Loading · Error}` segmented → drives
    `SampleData.state` (the selector from section 02) so the owner can see the
    empty "Нет встреч" / loading / error renderings in **both** variants.
- If wiring the data-state picker into the VM `StateFlow` proves fiddly, drop
  the picker and instead expose a documented **compile-time fixture flag**
  (`SampleData.state = …` edited in source) — and state in `PROTOTYPE.md`
  which approach was taken (the acceptance criterion accepts either).
- The bar must be visible in both variants. In the Jewel/decorated path it
  sits per the Jewel sample's content area; in the Material path it sits above
  `AppContent` (respecting the `FrameWindowScope` caveat from section 02 — if
  `AppContent` needs window scope, keep it inside the `Window {}` lambda).

### Step 2 — Window choice (D4) + macOS fallback

In `prototype/PrototypeRoot.kt`, finalize the window selection that section 02
skeletoned:

- `const val USE_DECORATED_WINDOW = true`.
- If `variant == Jewel && USE_DECORATED_WINDOW`: render Jewel's
  `DecoratedWindow` + `TitleBar` (per the pinned version's sample) hosting
  `JewelTheme { JewelAppShell(root) }` and the toggle bar.
- Else: render a standard Compose `Window` hosting either `AppContent(root)`
  (Material) or `JewelTheme { JewelAppShell(root) }` (Jewel) plus the toggle
  bar.
- Setting `USE_DECORATED_WINDOW = false` is the **macOS fallback**: Jewel
  *content* in a standard `Window`, no decorated chrome — used if the
  decorated window misbehaves on the owner's Mac (no native traffic-lights,
  etc.). Document this switch in `PROTOTYPE.md`.
- Crossing the `DecoratedWindow`↔`Window` boundary recreates the window
  **in-process** — a brief re-create, **no JVM restart**; the remount re-runs
  `LaunchedEffect`s (fixtures reload — harmless). If the owner finds the
  flicker jarring, the documented single-window alternative is: always host in
  `DecoratedWindow` and **hide `TitleBar` when variant == Material**. Note
  this option in `PROTOTYPE.md`.

### Step 3 — Verify the comparison works

Confirm (and have the owner confirm visually) that, launching default = Jewel:
flipping Material↔Jewel works both ways on **both** screens, navigation
MeetingList↔Settings works in both variants, the data-state control reaches
empty/loading/error, and there is no JVM restart.

### Step 4 — Write `ui-compose/PROTOTYPE.md`

The branch README. Contents:

- **Purpose** (one paragraph) + a prominent **"throwaway — never merge,
  never push unless asked"** warning.
- **Build/run commands**, exact:
  ```bash
  git checkout proto/jewel-look-feel
  cd ui-compose
  ./gradlew :desktopApp:compileKotlinDesktop   # must succeed
  ./gradlew :desktopApp:run                    # fake data, opens the window
  ```
  Explicitly state: **no Rust build, no `initCore`, no `ANTHROPIC_API_KEY`,
  no `run-compose.sh`** — runs on a bare JDK.
- **Final resolved version pins actually used**: CMP, Kotlin,
  compose-compiler, the **exact Jewel coordinate** (version + IJP build
  suffix), Decompose, coroutines, Gradle wrapper; and **every** dependency
  bumped vs `main`.
- **Any screen stubbed/excluded** under D7 (e.g. the markdown-renderer
  Protocol-detail screen) and why.
- **How the toggle works**; default = Jewel; the data-state control (or the
  compile-time fixture flag if the picker was dropped).
- The **`USE_DECORATED_WINDOW`** macOS fallback and the single-window
  anti-flicker option.
- Whether the **§B4 last-resort custom-theme fallback** was taken in
  section 01 (if so, the comparison is custom-theme-vs-Material, not
  Jewel-vs-Material).
- Any Jewel-vs-Material control substitutions noted in sections 03/04.
- The **verdict template** for the owner (section 06):
  ```
  MeetingList — verdict: [ desktop / still mobile / inconclusive ] — notes:
  Settings    — verdict: [ desktop / still mobile / inconclusive ] — notes:
  Overall stack decision: [ stay on Compose + restyle / migrate to Qt ]
  ```

## Acceptance Criteria

- [ ] `VariantToggleBar` always visible with a `{Material|Jewel}` control;
      flipping it changes variant live, **no JVM restart**, on both screens.
- [ ] A data-state control reaches Populated/Empty/Loading/Error in both
      variants — **or** a documented compile-time fixture flag is provided
      instead and `PROTOTYPE.md` says which.
- [ ] Default on launch is **Jewel**.
- [ ] Window choice wired: Jewel → `DecoratedWindow` + `TitleBar`; Material →
      standard `Window`; `USE_DECORATED_WINDOW=false` keeps Jewel content in a
      standard `Window` (macOS fallback) — all per the pinned Jewel sample.
- [ ] Navigation MeetingList↔Settings works in **both** variants.
- [ ] `ui-compose/PROTOTYPE.md` exists with: purpose + never-merge/never-push
      warning; exact build/run commands incl. the "no Rust/FFI/API key" note;
      final resolved version pins incl. exact Jewel coordinate; every dep
      bumped; any stubbed screen; toggle + data-state behavior; the
      `USE_DECORATED_WINDOW` macOS fallback + single-window anti-flicker
      option; whether §B4 fallback was taken; control substitutions; the
      verdict template.
- [ ] No edits to `shared` or the Rust core; prototype code isolated under
      `prototype/`.

## Files to Create/Modify

**Create:**

- `ui-compose/PROTOTYPE.md` — the branch README / handoff doc.
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/VariantToggleBar.kt`
  (optional — may live inside `PrototypeRoot.kt`).

**Modify:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/PrototypeRoot.kt` —
  finalize the toggle bar, `USE_DECORATED_WINDOW` window choice, macOS
  fallback.

**Do not modify:** `shared`, the Rust core.


---

# EMBEDDED: sections/section-06-verdict-closeout.md

# Section 06: Verdict & Decision Close-out (Owner-Driven)

## Background

**Meeting Assistant** is a Rust + Kotlin Multiplatform Compose Desktop app
(`/ui-compose`). Its UI uses stock Material 3 and the owner feels it "reads as
mobile" — the single remaining argument for abandoning the
Rust + Kotlin/Compose stack for **Qt**. Every other Qt argument was already
refuted (QtMultimedia doesn't solve macOS system audio; installer/updater are
framework-agnostic via jpackage + Sparkle; the current permissive license
stack is strictly more flexible than Qt's).

Sections 01–05 delivered a **throwaway, never-merged** prototype on branch
`proto/jewel-look-feel`: MeetingList + Settings re-rendered with JetBrains
Jewel (on an upgraded CMP 1.10 / Kotlin 2.1.x toolchain), toggled at runtime
against the unmodified Material 3 screens, running on fake data with no FFI,
plus `ui-compose/PROTOTYPE.md` documenting build/run and a verdict template.

This is the **purpose of the entire exercise**: convert the prototype into a
**stack decision**. It is **owner-driven** — the assistant cannot launch a
desktop GUI, so the owner runs the build, looks at both variants, and renders
the verdict; the assistant records the outcome and closes the question.

## Requirements

When this section is complete:

- The owner has built, run, and toggled both screens (Material ↔ Jewel) and
  filled the per-screen verdict template in `ui-compose/PROTOTYPE.md`.
- The resulting stack decision is recorded in project memory with a date.
- The throwaway branch is left **unmerged and local** (not deleted, not
  pushed unless the owner explicitly asked).

## Dependencies

- **Requires:** section 05 (runnable comparison build + `PROTOTYPE.md`).
- **Blocks:** nothing — this is terminal.

## Implementation Details

### Step 1 — Owner runs the comparison

The owner (decision-maker) performs the visual evaluation the assistant
cannot:

```bash
git checkout proto/jewel-look-feel
cd ui-compose
./gradlew :desktopApp:compileKotlinDesktop
./gradlew :desktopApp:run
```

The owner toggles Material ↔ Jewel on **both** MeetingList and Settings,
exercises the data-state control (populated/empty/loading/error), navigates
between screens in both variants, and judges each screen subjectively:
*"this reads as a desktop app, not mobile"* — or not. The Settings screen
(dense form) is the key stress point.

### Step 2 — Record the per-screen verdict

Fill the verdict template in `ui-compose/PROTOTYPE.md`:

```
MeetingList — verdict: [ desktop / still mobile / inconclusive ] — notes:
Settings    — verdict: [ desktop / still mobile / inconclusive ] — notes:
Overall stack decision: [ stay on Compose + restyle / migrate to Qt ]
```

If the §B4 last-resort fallback was taken in section 01, note that the
comparison was custom-compact-theme-vs-Material (a slightly weaker
"proven-native" signal than real Jewel) and weigh the verdict accordingly.

### Step 3 — Close the Qt-vs-Compose decision

Based on the recorded verdict, take exactly one branch:

- **(a) "theming fixes it"** (Jewel/custom reads as desktop) → **stay on the
  Compose stack; drop Qt.** Open a *real* (non-throwaway) work item to
  restyle the production app in-place — either adopt Jewel properly (which
  means the production app also moves to the CMP 1.10 / Kotlin 2.1 toolchain
  — scope that as its own task, including the FFI/UniFFI revalidation the
  prototype skipped) or implement the custom compact desktop theme on the
  current toolchain.
- **(b) "still not desktop enough"** → Qt's look argument **stands**. Open a
  Qt-migration plan (the open question "audio in Rust vs C++" from project
  memory becomes active).

### Step 4 — Update project memory

Update the project memory so the decision persists across sessions:

- Update the memory note tracking the **Qt-vs-Compose open question** /
  `project_goal_shipping` with: the dated verdict (per-screen + overall),
  which branch (a/b) was taken, and whether the §B4 fallback influenced it.
- Convert any relative dates to absolute. Link related memories.
- If outcome (a): note that Qt is dropped and an in-place restyle task is the
  follow-up (and that adopting Jewel for real implies a production toolchain
  upgrade). If outcome (b): note the Qt-migration plan is now open.

### Step 5 — Leave the branch alone

- `proto/jewel-look-feel` remains **unmerged** and **local**. Do not merge,
  do not delete, do not push (unless the owner explicitly asks). Its value
  was the verdict, now recorded; the code is reference, not product.

## Acceptance Criteria

- [ ] Owner has built and run the prototype with the documented commands and
      toggled both screens between Material and Jewel without further
      instruction.
- [ ] An explicit **per-screen verdict** (MeetingList, Settings) and an
      overall stack decision are recorded in `ui-compose/PROTOTYPE.md`.
- [ ] The Qt-vs-Compose decision is closed: either (a) stay on Compose + a
      real in-place restyle work item is identified and Qt dropped, or (b) a
      Qt-migration plan is opened.
- [ ] Project memory updated with the dated decision (per-screen + overall,
      branch a/b, §B4-fallback influence), relative dates made absolute.
- [ ] Branch `proto/jewel-look-feel` left unmerged and local (not deleted,
      not pushed unless explicitly asked).

## Files to Create/Modify

**Modify:**

- `ui-compose/PROTOTYPE.md` — fill in the per-screen verdict template +
  overall stack decision (owner).
- Project memory (the user's auto-memory directory) — record the dated
  Qt-vs-Compose decision and the follow-up (restyle task or Qt-migration
  plan).

**Do not modify:** any prototype/`shared`/Rust code — this section produces a
**decision**, not code. The branch stays as-is, unmerged.


---

## Completion

When all six embedded sections above are implemented and every Acceptance
Criterion in each is verified, emit on its own line:

<promise>ALL-SECTIONS-COMPLETE</promise>
