# Internal Critical Review

**Model:** claude-opus-4-7 (adversarial self-review — gemini & codex CLIs not installed)
**Generated:** 2026-05-17 23:20

---

Reviewing `claude-plan.md` as an unaffiliated senior architect. The plan is
sound in its strategy (upgrade-then-Jewel, fakes, two screens, throwaway).
The risks below are real and several are correctness blockers, not polish.

## A. Correctness blockers (must address before Phase 1)

### A1. Decompose lifecycle wiring is omitted — ViewModels may never init
The plan's `PrototypeMain.kt` builds `RootComponent(DefaultComponentContext(
LifecycleRegistry()), …)` but **never resumes the lifecycle**. `RootComponent`
creates ViewModels via `instanceKeeper.getOrCreate`, and `MeetingListViewModel`
runs `init { load() }`. If the Decompose lifecycle is not moved to
`RESUMED` (and, on desktop, bound to the window via `LifecycleController`),
`InstanceKeeper` instances and `init` side-effects may not behave as in
`Main.kt`. **Action:** the plan must instruct the implementer to **replicate
`Main.kt`'s exact lifecycle setup** (`LifecycleRegistry`,
`lifecycle.resume()` / `LifecycleController(lifecycle, windowState)` —
whatever `Main.kt` actually does) in `PrototypeMain.kt`. This is the single
most likely "screens render blank" footgun.

### A2. `RootComponent` / repository interface signatures are assumed
Constructor parameter order/names, the exact repository interface FQNs, and
that `meetingListViewModel`/`recordingViewModel` are `public` are taken from
research, not verified. **Action:** Phase 1 must begin by reading
`RootComponent.kt` and each repository interface and matching the fake
implementations to the *actual* suspend signatures (return types, nullability,
exceptions) before writing fakes.

### A3. `compose.materialIconsExtended` may not exist in CMP 1.10
`shared/build.gradle.kts` declares `compose.materialIconsExtended`. The
material-icons-extended artifact has been repackaged/deprecated across
Compose Multiplatform releases; on CMP 1.10 this DSL accessor or artifact may
be gone, which would break the **existing Material screens** (the comparison
baseline) — not just Jewel. **Action:** add an explicit Phase-0 check: does
`compose.materialIconsExtended` resolve on CMP 1.10? If not, either add the
standalone `material-icons-extended` coordinate or replace extended-icon
usages. This belongs in the hard gate, because a broken Material baseline
voids the whole comparison.

### A4. Jewel `DecoratedWindow` / `TitleBar` structural contract
The snippet places `TitleBar { } ; toggle() ; shell()` inside a plain
`Column` inside `DecoratedWindow`. Jewel's `DecoratedWindow` exposes a
`DecoratedWindowScope` and `TitleBar` is expected as a specific slot/child,
not an arbitrary `Column` element; misuse compiles but renders wrong or
throws. **Action:** the plan must say "follow the pinned Jewel version's
`DecoratedWindow`/`TitleBar` sample exactly; do not assume the Column layout
in §2 is API-correct — it is illustrative." Treat §2's Kotlin as pseudocode.

## B. High-impact risks

### B1. Cyrillic glyph coverage in Jewel's default font
Every label is Russian. If Jewel's bundled Int UI font lacks Cyrillic glyphs,
the Jewel screens render tofu boxes and the owner's verdict is invalidated by
a font bug, not a look-and-feel signal. **Action:** add an explicit
acceptance check "Russian labels render correctly in the Jewel variant" and a
mitigation note (provide a Cyrillic-capable `FontFamily` to the Jewel
`TextStyle`/theme if needed).

### B2. `runPrototype` JavaExec task is under-specified and fights the plugin
Hand-rolling a `JavaExec` against a Kotlin/Compose Multiplatform desktop
runtime classpath is fragile (target/classpath resolution, Compose resources,
skiko natives). The Compose Desktop Gradle plugin really supports one
`application.mainClass`. **Action:** make the **mainClass-switch the primary
documented path** (point `application.mainClass` at `PrototypeMainKt` on the
throwaway branch — acceptable since it's never merged and the in-app toggle
still reaches Material), and demote the `JavaExec` task to "only if you can
make it resolve". This removes a likely multi-hour yak-shave.

### B3. Decompose ↔ CMP version skew
Decompose `extensions-compose` transitively depends on a Compose version;
3.3.x against CMP 1.10 + Kotlin 2.1 can drag an incompatible Compose runtime
(classic Skiko/runtime-skew crash). **Action:** Phase 0 must include an
explicit dependency-tree alignment check for Decompose's Compose pull, and
authorize forcing/aligning it (the branch is throwaway).

### B4. Reusing `AppContent(root)` may assume window/Frame scope
`AppContent` (and `Sidebar`/`ContentPane`) may rely on `FrameWindowScope` or
window CompositionLocals. Nesting it inside our `Column`/`Window` should be
fine, but if it declares a `MenuBar` or uses window-scoped APIs it will fail
outside the expected scope. **Action:** verify `AppContent`'s scope
requirements during Phase 1; if it needs `FrameWindowScope`, call it directly
inside the `Window {}` lambda (which is a `FrameWindowScope`) rather than
wrapping in an outer non-window `Column`.

## C. Gaps & ambiguities

### C1. No UI path to non-default data states
Acceptance requires Loading/Error/empty ("Нет встреч") states "covered", but
the toggle UX exposes only Material↔Jewel — there is no affordance to reach
those states, and fakes return success instantly. **Action:** either add a
tiny dev control (state picker: Populated / Empty / Loading / Error) or
explicitly downgrade the acceptance criterion to "populated + empty via a
compile-time fixture switch, documented in PROTOTYPE.md". Don't claim states
that can't be reached.

### C2. Last-resort custom-theme fallback is a cliff
If Phase 0's Jewel gate fails, the plan says "implement a custom compact
theme" with no recipe. `claude-research.md` §B4 has the concrete recipe
(tokens, ~28–32.dp controls, ~6–8.dp spacing, ~13–14.sp text, hover). 
**Action:** the fallback must explicitly reference research §B4 so it is
actionable, and the section files must carry that content inline (sections
are self-contained).

### C3. Window recreation on every variant flip
Crossing the `DecoratedWindow`↔`Window` boundary recreates the window and
remounts the composition, re-running `LaunchedEffect`s (Settings reloads
fixtures, list reloads). Functionally OK but the "live, no restart" criterion
should be clarified to "no JVM restart; a brief window re-create on variant
switch is expected." Also offer the documented single-window alternative
(always `DecoratedWindow`, hide `TitleBar` in Material) to avoid the flicker
if the owner finds it jarring.

### C4. Branch delivery / git hygiene unstated
The plan doesn't say whether the branch is pushed. Per repo norms the
assistant should **not** push without explicit ask; deliverable = local
branch + commits. State this so the executor doesn't auto-push a throwaway
branch.

## D. Minor

- D1. `PROTOTYPE.md` should record the **exact resolved** Jewel coordinate
  (version + IJP build suffix), not just "0.36 line".
- D2. The "no new compiler warnings in Material paths" criterion is already
  softened in the plan — ensure the **section files** carry the softened
  wording, not the original PRD's absolute one.
- D3. Add a quick `git switch -` / branch-untouched assertion to acceptance
  (prove `main` and other branches have no new commits).
- D4. Consider pinning Gradle explicitly (e.g. 8.13) in Phase 0 rather than
  "bump only if Kotlin demands" — discovering the wrapper is too old mid-build
  is a slow failure; deciding up front is cheaper.

## Bottom line

Strategy: approved. Before implementation the plan needs **A1–A4** folded in
(lifecycle wiring, signature verification, materialIconsExtended check, Jewel
API-shape caveat) — these are the difference between "screens render" and "a
day lost to blank windows". B1 (Cyrillic) and B2 (run task) are cheap to
address now and expensive to discover late. The rest are clarifications that
make the section files honest.
