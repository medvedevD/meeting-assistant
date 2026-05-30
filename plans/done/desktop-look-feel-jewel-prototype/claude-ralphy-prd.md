# PRD: Desktop Look-and-Feel (Jewel) Prototype

A throwaway, never-merged prototype that re-renders the MeetingList and
Settings screens of the Meeting Assistant Compose Desktop app with JetBrains
Jewel (on an upgraded CMP 1.10 / Kotlin 2.1.x toolchain), toggled at runtime
against the existing Material 3 screens, so the owner can decide "desktop vs
mobile" and close the Qt-vs-Compose stack question.

## How to use

```bash
ralphy --prd .claude/plans/desktop-look-feel-jewel-prototype/claude-ralphy-prd.md
# or: cp .claude/plans/desktop-look-feel-jewel-prototype/claude-ralphy-prd.md ./PRD.md && ralphy
```

## Context

- **Repo root:** `/Users/dmitrymedvedev/projects/pets/meeting-assistant`
- **Section files** (each is fully self-contained — read the section file
  before implementing it):
  `.claude/plans/desktop-look-feel-jewel-prototype/sections/`
- **Index + dependency graph:** `sections/index.md`
- **Background/plan (reference only):** `claude-plan.md`, `claude-spec.md`,
  `claude-research.md` (the §B4 fallback recipe is also embedded in
  section 01).

## Execution rules

1. Implement sections **in dependency order** (see `sections/index.md`):
   01 → 02 → (03 ∥ 04) → 05 → 06.
2. **Section 01 is a HARD GATE** — do not start any other section until it
   passes (toolchain resolves, Material baseline intact, Jewel smoke renders
   OR the §B4 custom-theme fallback is recorded).
3. Each section file is authoritative and self-contained. Read it, build a
   TODO from its Acceptance Criteria, implement, then verify every criterion.
4. **Isolation:** never edit `shared` ViewModels/domain or the Rust core
   except the sanctioned Gradle/version-catalog/run-config changes and (only
   if forced) a stub of a *non-target* screen. The Material comparison is the
   **unmodified** `AppContent(root)`.
5. The branch `proto/jewel-look-feel` stays **local and unmerged** — never
   push unless the owner explicitly asks.
6. Sections 01 (smoke run) and 06 (verdict) require the **owner** to run the
   desktop GUI; the agent delivers a compiling branch + exact commands and
   stops for owner action where the section says so.

## Tasks

- [ ] Section 01: Toolchain Upgrade + Jewel Compatibility Gate (HARD GATE)
- [ ] Section 02: Fakes + Dev Entry Point + Material Variant Runs
- [ ] Section 03: Jewel MeetingList Screen + Jewel Theme/Shell
- [ ] Section 04: Jewel Settings Screen (Form-Density Stress Point)
- [ ] Section 05: Runtime Toggle + Decorated Window + macOS Fallback + README
- [ ] Section 06: Verdict & Decision Close-out (Owner-Driven)
