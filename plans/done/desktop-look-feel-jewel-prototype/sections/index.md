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
