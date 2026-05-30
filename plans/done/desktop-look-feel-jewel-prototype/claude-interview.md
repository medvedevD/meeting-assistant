# Interview Transcript — Desktop Look-and-Feel (Jewel) Prototype

Date: 2026-05-17. Conducted after research, which surfaced the headline
finding that **no Jewel release supports CMP 1.7.3 / Kotlin 2.0.21**.

## Round 1 — Core forks

### Q1. Jewel path (given Jewel ≠ CMP 1.7.3 / Kotlin 2.0.21)
**Answer: Upgrade toolchain, use real Jewel.**
Bump CMP 1.7.3 → 1.10.0 and Kotlin 2.0.21 → 2.1.x on the throwaway branch so
genuine Jewel (0.35+) works. Strongest "proven-native" signal. Accepts the
Skiko/FFI risk and supersedes the PRD's "no toolchain change" constraint *on
this throwaway branch only*.

### Q2. Data source
**Answer: Fake repos, no Rust/FFI.**
Implement fake `MeetingRepository` / `SettingsRepository` with representative
static data. Dev composition root skips `initCore()` and the Rust dylib.
Prototype runs with only a JDK — removes FFI/native env risk for a purely
visual test.

### Q3. Toggle UX + default
**Answer: In-window control, default new variant.**
A visible, always-on in-window control. App launches showing the **Jewel**
variant first (puts "is it desktop?" front-and-center); one click flips to
Material 3 and back, no restart.

### Q4. Fidelity
**Answer: Match content & density, not pixels.**
Variant screens must show the same fields/controls/data and a desktop-
appropriate density, but are free to use idiomatic desktop sizing/spacing.
The point is judging the look, not cloning Material pixel-for-pixel. Scope =
the two screens (MeetingList + Settings).

## Round 2 — Risk & boundaries

### Q5. Toolchain-upgrade fallback if it cascades
**Answer: Bump whatever's needed.**
Accept wider dependency churn on the throwaway branch — bump Decompose,
markdown-renderer-m3, coroutines, compose-compiler, etc. to whatever versions
make CMP 1.10 + Kotlin 2.1 + Jewel compile and run. Branch is never merged, so
blast radius is zero. (No automatic fall-back to custom theme; push the
upgrade through.)

### Q6. Window chrome
**Answer: Jewel DecoratedWindow + TitleBar.**
The Jewel variant uses Jewel's custom window chrome (the Material variant keeps
the standard `Window`). Chrome is part of the "reads as desktop" signal.
Accept macOS quirks (no exact native traffic-lights); fall back to a standard
window *keeping Jewel content* only if the decorated window misbehaves on the
owner's macOS.

### Q7. Compile scope on the upgraded branch
**Answer: Only the comparison must work.**
Guarantee only that MeetingList + Settings render in both Material and Jewel
variants with fake data. If an unrelated screen blocks the CMP/Kotlin upgrade,
it may be excluded/stubbed on this throwaway branch. Fastest path to a verdict.

### Q8. Branch + verification ownership
**Answer: New throwaway branch off current.**
Create a dedicated throwaway branch off `feat/compose-desktop-rewrite` (e.g.
`proto/jewel-look-feel`); never merged. Assistant makes it build and documents
exact `./gradlew` commands; the owner runs the desktop GUI and renders the
verdict (the assistant cannot launch a desktop UI).

## Architect's closing summary (confirmed by answers)

- Throwaway branch off `feat/compose-desktop-rewrite`, never merged.
- Toolchain upgraded **on the branch**: CMP 1.10.0, Kotlin 2.1.x, matching
  compose-compiler; bump Decompose / markdown-renderer / coroutines as needed.
- Real Jewel (`jewel-int-ui-standalone` + `jewel-int-ui-decorated-window`,
  latest 0.35+/0.36 line for CMP 1.10).
- Two screens reimplemented in Jewel: MeetingList (sidebar list) + Settings
  (form-density stress point); content & density faithful, not pixel-faithful.
- Fake repositories with static data; dev composition root, **no `initCore`,
  no Rust dylib** — runs on a bare JDK.
- Jewel variant uses `DecoratedWindow` + `TitleBar`; macOS standard-window
  fallback keeping Jewel content if needed. Material variant keeps standard
  `Window`.
- Visible in-window toggle; **default = Jewel on launch**; flips live, no
  restart; navigation MeetingList↔Settings works in both variants.
- Only the comparison must compile/run; unrelated screens may be stubbed/
  excluded on the branch.
- Owner builds, runs, toggles, and records the per-screen verdict; assistant
  delivers branch + exact build/run commands + README.
