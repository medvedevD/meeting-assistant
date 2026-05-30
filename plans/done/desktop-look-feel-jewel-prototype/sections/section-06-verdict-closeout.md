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
