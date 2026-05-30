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
