# Desktop Look-and-Feel (Jewel) Prototype - Product Requirements Document (PRD)

## Requirements Description

### Background
- **Business Problem**: The current Compose Desktop UI uses stock Material 3 and reads as
  "mobile-like". This is the single remaining substantive argument for abandoning the
  Rust + Kotlin/Compose stack in favor of Qt. Every other Qt argument has been
  refuted in discussion (QtMultimedia does not solve macOS system audio;
  installer/updater are framework-agnostic via jpackage + Sparkle; the current
  permissive license stack is strictly more flexible than Qt's LGPL/GPL/commercial).
- **Target Users**: The project owner, as the decision-maker, evaluating the stack.
- **Value Proposition**: A cheap, empirical test that determines whether the
  "mobile feel" is a *theming choice* (fixable in-place, no rewrite) or a
  *stack limitation* (justifies a Qt migration). Avoids paying for a third UI
  rewrite to fix something that may be a styling problem.

### Feature Overview
- **Core Features**: A throwaway, dev-only prototype on a dedicated branch that
  re-implements two existing screens (MeetingList, Settings) using JetBrains
  **Jewel** components, switchable at runtime against the current Material 3
  versions for side-by-side comparison.
- **Feature Boundaries**:
  - IN: Jewel reimplementation of MeetingList + Settings; a runtime dev toggle
    (Material ↔ Jewel); native-ish window chrome where Jewel provides it;
    representative data so screens render fully.
  - OUT: production integration; other screens; behavior/logic changes;
    persistence of the toggle; packaging/signing; macOS system audio; any
    decision about Qt (that is the *outcome* of this test, not part of it).
- **User Scenarios**: Owner builds the branch locally, launches the app, toggles
  between Material and Jewel on both screens, and renders a subjective verdict:
  "this reads as a desktop app, not mobile" — or not.

### Detailed Requirements
- **Input/Output**: Input = existing MeetingList/Settings UI + their ViewModels.
  Output = a runnable desktop app exposing both Material and Jewel renderings of
  those two screens behind a visible dev toggle.
- **User Interaction**: A dev-only control (e.g., a toolbar switch or menu item,
  gated by a build flag/env var) flips the active screen set between Material and
  Jewel without restart. Navigation between MeetingList and Settings works in
  both modes.
- **Data Requirements**: Reuse real ViewModels/repositories if they initialize
  cleanly; otherwise feed representative static data so layout density is
  faithfully shown (the test is visual, not functional). No schema changes.
- **Edge Cases**:
  - Jewel artifact does not resolve / conflicts with Compose 1.7.3 or Kotlin
    2.0.21 → invoke the documented fallback (custom compact non-Material theme,
    same two screens, same toggle) so the experiment still yields a verdict.
  - Jewel decorated-window unsupported on the dev OS → fall back to standard
    window, keep Jewel content.
  - ViewModel init requires native FFI/DB → use static representative data.

## Design Decisions

### Technical Approach
- **Architecture Choice**: Isolated dev-only layer. No changes to `shared`
  domain/ViewModels or the Rust core. Jewel screens live in a separate package
  and are selected at composition root via a build/dev flag. This keeps the
  prototype throwaway and the main app unaffected.
- **Key Components**:
  - Jewel dependency (`org.jetbrains.jewel` standalone int-ui artifacts) pinned
    to a version compatible with Compose Multiplatform 1.7.3 / Kotlin 2.0.21.
  - `JewelMeetingListScreen`, `JewelSettingsScreen` — Jewel-component
    reimplementations mirroring the current screens' layout/content.
  - A `UiVariant` switch (Material | Jewel) read at the composition root, with a
    visible dev toggle.
- **Data Storage**: None. No new persistence; the toggle is in-memory/dev only.
- **Interface Design**: No public API/FFI changes. Internal only.

### Constraints
- **Performance Requirements**: None beyond "renders without jank on the dev
  machine". This is a visual test.
- **Compatibility**: Must build with the existing Compose Multiplatform 1.7.3 /
  Kotlin 2.0.21 toolchain. Jewel version selection is gated by this.
- **Security**: None (dev-only, no new inputs, no distribution).
- **Scalability**: Explicitly throwaway; not designed to extend.

### Risk Assessment
- **Technical Risk — Jewel ↔ Compose version compatibility (PRIMARY)**: Jewel
  standalone artifacts track specific IntelliJ-platform/Compose/Kotlin
  combinations. With CMP 1.7.3 / Kotlin 2.0.21 there is real risk of resolution
  or runtime friction. *Mitigation*: Phase 1 is a hard gate — a dependency spike
  that must resolve and render a trivial Jewel window before any screen work. If
  it cannot be made to work within a short timebox, switch this same experiment
  to the custom compact non-Material theme (same two screens, same toggle); the
  comparison's value is preserved (it still answers "theming vs stack limit",
  just with a slightly weaker "proven-native" signal).
- **Dependency Risk**: Jewel is the only new dependency and is confined to the
  throwaway branch — zero blast radius on `main`/feature branches.
- **Schedule Risk**: Scope creep into "make it production-ready". *Mitigation*:
  hard boundary — two screens, visual only, branch is never merged.
- **Environment Risk**: The build/run requires a local JDK (the assistant's
  sandbox has none). Verification of "it actually renders" is performed by the
  project owner on their machine; the assistant delivers the branch + exact
  build/run commands.

## Acceptance Criteria

### Functional Acceptance
- [ ] A dedicated throwaway branch contains the prototype; `main` and active
      feature branches are untouched.
- [ ] MeetingList is rendered in both Material (existing) and Jewel variants.
- [ ] Settings is rendered in both Material (existing) and Jewel variants.
- [ ] A visible dev toggle switches Material ↔ Jewel at runtime, no restart,
      for both screens; navigation works in both modes.
- [ ] If Jewel proved incompatible, the documented custom-theme fallback was
      implemented instead and this is clearly noted in the branch README.

### Quality Standards
- [ ] Prototype code is isolated in its own package; no edits to `shared`
      ViewModels/domain or the Rust core.
- [ ] Build succeeds on the owner's machine with the current toolchain; exact
      `./gradlew` / run commands are documented in the branch.
- [ ] No new compiler warnings introduced in the existing (Material) code paths.
- [ ] Branch README states what was built, how to run it, and the Jewel
      version used (or the fallback taken).

### User Acceptance
- [ ] Owner can launch the app and toggle both screens between Material and
      Jewel without further instruction.
- [ ] Owner renders an explicit verdict on each screen: "desktop, not mobile"
      vs "still mobile / not enough".
- [ ] The verdict is recorded so the Qt-vs-Compose decision can be closed.

## Execution Phases

### Phase 1: Jewel Compatibility Spike (HARD GATE)
**Goal**: De-risk the primary technical risk before any screen work.
- [ ] Create throwaway branch off the current branch.
- [ ] Add Jewel standalone dependency; resolve a version compatible with
      CMP 1.7.3 / Kotlin 2.0.21.
- [ ] Stand up a trivial Jewel window (a label + a button) and confirm it
      builds and renders on the owner's machine.
- [ ] Decision point: Jewel works → Phase 2. Jewel cannot be made to work in
      the timebox → switch to custom compact non-Material theme for the same
      two screens, note it, continue to Phase 2 with that approach.
- **Deliverables**: Branch + resolving Jewel (or recorded fallback decision).
- **Time**: ~0.5 day (owner runs the build check).

### Phase 2: Jewel Reimplementation of Two Screens
**Goal**: Faithful Jewel versions of MeetingList and Settings.
- [ ] `JewelMeetingListScreen` mirroring layout/content of the Material one.
- [ ] `JewelSettingsScreen` mirroring the Material one (form density is the key
      stress point — match field/control sizing to a desktop idiom).
- [ ] Feed real ViewModels or representative static data so both render fully.
- **Deliverables**: Two Jewel screens compiling on the branch.
- **Time**: ~1–1.5 days.

### Phase 3: Runtime Toggle & Comparison Harness
**Goal**: Side-by-side comparability.
- [ ] `UiVariant` switch at the composition root, default Material.
- [ ] Visible dev toggle (toolbar/menu) flipping the variant live for both
      screens; navigation works in both.
- [ ] Branch README: build/run commands, Jewel version (or fallback), what to
      look at.
- **Deliverables**: Runnable comparison build.
- **Time**: ~0.5 day.

### Phase 4: Verdict & Decision Close-out
**Goal**: Convert the prototype into a stack decision.
- [ ] Owner builds, runs, toggles, and renders the per-screen verdict.
- [ ] Record outcome: (a) "theming fixes it" → stay on Compose, plan an
      in-place Jewel/custom restyle as real work, drop Qt; or (b) "still not
      desktop enough" → Qt's look argument stands, open a Qt-migration plan.
- [ ] Update project memory with the decision.
- **Deliverables**: Recorded decision; branch remains unmerged (throwaway).
- **Time**: ~0.5 day (owner-driven).

---

**Document Version**: 1.0
**Created**: 2026-05-17
**Clarification Rounds**: 2
**Quality Score**: 91/100
