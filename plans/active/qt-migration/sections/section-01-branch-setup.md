# Section 01 — Branch setup

## Background
The Qt-vs-Compose verdict was reached on the throwaway prototype branch
`proto/jewel-look-feel` (local-only, never pushed). Its only value — the
verdict — is now fully captured in project memory and in this plan. The owner
instructed (2026-05-18) to delete it and implement the Qt migration on a fresh
branch. This **consciously reverses** the Section-06 close-out decision ("leave
the prototype branch unmerged and local — not deleted"); the reversal is
deliberate and safe.

## Requirements (done when true)
- A new branch `feat/qt-migration` exists, taken from the production-Compose
  base so that `ui-compose/` is present (it is the behavior reference for the
  QML screens section).
- The `.claude/plans/qt-migration/` planning files are present on the new branch.
- Any artifact that existed ONLY on `proto/jewel-look-feel` and must survive is
  salvaged first.
- `proto/jewel-look-feel` no longer exists locally.
- Nothing pushed to any remote.

## Dependencies
- Requires: nothing (FIRST action, before any code).
- Blocks: all other sections.

## Implementation details
1. **Salvage.** The only prototype-only artifact of value is the filled verdict
   in `ui-compose/PROTOTYPE.md`. Its content is already mirrored in project
   memory (`project_goal_shipping.md`). If a repo copy must persist, copy
   `PROTOTYPE.md` into `.claude/plans/qt-migration/` before deletion.
2. **Identify the base.** `ui-compose/` (production Compose UI, the behavior
   reference) lives on `feat/compose-desktop-rewrite`. Confirm at execution:
   `git branch` shows `feat/compose-desktop-rewrite`, `main`,
   `proto/jewel-look-feel`. Branch off `feat/compose-desktop-rewrite` unless the
   owner says otherwise.
3. **Create the implementation branch:**
   `git checkout feat/compose-desktop-rewrite`
   `git checkout -b feat/qt-migration`
4. **Ensure planning files present** on `feat/qt-migration` (copy the
   `.claude/plans/qt-migration/` directory over if it was authored on another
   branch).
5. **Delete the prototype branch** (only after 1–4 verified; cannot delete the
   checked-out branch, so run from `feat/qt-migration`):
   `git branch -D proto/jewel-look-feel`
6. Do **not** push anything unless the owner explicitly asks.

## Acceptance criteria
- [ ] `feat/qt-migration` exists, created from the correct Compose base.
- [ ] `ui-compose/` is present on `feat/qt-migration`.
- [ ] `.claude/plans/qt-migration/` is present on `feat/qt-migration`.
- [ ] PROTOTYPE.md verdict salvaged (in project memory; optionally copied into
      the planning dir).
- [ ] `git branch` no longer lists `proto/jewel-look-feel`.
- [ ] No remote was pushed.

## Files to create/modify
- Git branches only (no source changes).
- Optional: copy `ui-compose/PROTOTYPE.md` → `.claude/plans/qt-migration/`.
