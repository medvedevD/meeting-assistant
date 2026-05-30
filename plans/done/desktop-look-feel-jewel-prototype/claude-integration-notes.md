# Integration Notes

Source: `reviews/internal-critical-review.md` (gemini & codex CLIs not
installed; substituted a rigorous adversarial self-review per user choice).

I am the authority on what to integrate. Verdict: the review is mine and
high-signal; **integrate A1–A4, B1–B4, C1–C4, D1–D4** — essentially all of
it. Nothing rejected outright; two items are integrated with a narrower scope
than written.

## Integrated in full

| Item | Why | Where in plan |
|---|---|---|
| **A1** lifecycle wiring | Genuine "blank screens" blocker. The dev entry must replicate `Main.kt`'s Decompose lifecycle (LifecycleRegistry + resume / LifecycleController). | §2 control-flow + Phase 1 step + new Risk row |
| **A2** verify `RootComponent`/repo signatures first | Fakes against assumed signatures will not compile/behave. | Phase 1 step 0 ("read before write"); §6 |
| **A3** `compose.materialIconsExtended` on CMP 1.10 | A broken **Material baseline** voids the comparison — must be in the hard gate. | Phase 0 step + new Risk row |
| **A4** Jewel `DecoratedWindow`/`TitleBar` API shape | §2 Kotlin is illustrative; real API differs by version. | §2 caveat banner; Phase 2/4 instruction |
| **B1** Cyrillic glyph coverage | All labels Russian; a font bug would invalidate the verdict. | Phase 2 step + acceptance + Risk row |
| **B2** make mainClass-switch the primary run path | `JavaExec` against KMP/Compose classpath is a known time sink. | §2, Phase 1 step 4, §5 commands |
| **B3** Decompose ↔ CMP skew alignment | Classic Skiko runtime-skew crash vector. | Phase 0 dependency-alignment step + Risk row |
| **B4** verify `AppContent` scope needs | If it needs `FrameWindowScope`, our outer Column breaks it. | §2 note + Phase 1 step |
| **C1** no UI path to Loading/Error/empty | Acceptance claimed unreachable states. | Added a dev **state picker** to the toggle bar; acceptance reworded |
| **C2** fallback references research §B4 | Fallback must be actionable, not a cliff. | Phase 0 gate text + sections will embed §B4 recipe |
| **C3** clarify "no restart" + offer single-window option | Set correct expectation; give anti-flicker option. | §2, D5 wording, Risk row |
| **C4** git hygiene: do not auto-push | Repo norm; throwaway branch stays local unless owner asks. | §6 + Phase 0 + acceptance |
| **D1** record exact resolved Jewel coordinate | Reproducibility. | Phase 4 README contents |
| **D2** softened-warning wording into sections | Consistency PRD→sections. | §6 already softened; fl/forward to sections |
| **D3** assert other branches untouched | Cheap proof for a "throwaway" guarantee. | Acceptance |
| **D4** decide Gradle version up front | Avoid slow mid-build discovery. | Phase 0: pin Gradle wrapper explicitly |

## Integrated with narrowed scope

- **C1 (state picker):** integrate, but keep it *dead simple* — a second
  segmented control {Populated · Empty · Loading · Error} feeding the fake
  repos, not a settings panel. If it proves fiddly with the VM `StateFlow`,
  acceptance falls back to "Populated + Empty via a documented compile-time
  fixture flag" (the review's own escape hatch). Avoids scope creep.
- **B2 (run path):** make mainClass-switch primary and **drop the `JavaExec`
  task entirely** from the required deliverable (mention it only as an
  optional nicety). Reduces required surface; the branch is throwaway so a
  swapped `mainClass` is harmless.

## Not changing

- Overall strategy, phase structure, version table, isolation rules — the
  review explicitly approved these.
- No security/performance content added: this is a dev-only, no-network,
  no-FFI, never-merged visual prototype; the review correctly raised none.

Next: apply these edits to `claude-plan.md`.
