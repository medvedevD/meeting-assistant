# Prototype: Jewel Look & Feel (`proto/jewel-look-feel`)

> **THROWAWAY BRANCH — NEVER MERGE, NEVER PUSH UNLESS EXPLICITLY ASKED.**
> All code under `desktopApp/src/desktopMain/kotlin/prototype/` is evaluation-only and
> must be deleted before any production release of the `ui-compose` module.

## Purpose

This branch re-renders the **MeetingList** sidebar and **Settings** screen using JetBrains
Jewel (IntUI standalone) alongside the existing Material 3 implementation, so the project
owner can make a side-by-side visual comparison and decide whether Compose Desktop can
deliver a desktop-native look (IntelliJ-style) or whether a migration to Qt is needed.

The prototype is self-contained: it runs on a bare JDK with no Rust build, no FFI
initialisation, and no Anthropic API key. All data is served by in-process fake repositories.

---

## Build & run

```bash
git checkout proto/jewel-look-feel
cd ui-compose
./gradlew :desktopApp:compileKotlinDesktop   # must succeed cleanly
./gradlew :desktopApp:run                    # opens the window with fake data
```

**No Rust build, no `initCore`, no `ANTHROPIC_API_KEY`, no `run-compose.sh` required.**
The prototype runs on a bare JDK 21+.

---

## Final resolved version pins

| Dependency | Branch version | Base (`feat/compose-desktop-rewrite`) |
|---|---|---|
| Kotlin | **2.3.10** | 2.0.21 |
| Compose Multiplatform (CMP) | **1.10.0** | 1.7.3 |
| Compose compiler | **2.3.10** (= Kotlin) | 2.0.21 |
| **Jewel IntUI standalone** | **`org.jetbrains.jewel:jewel-int-ui-standalone:0.36.0-261.24374.151`** | — (new) |
| **Jewel IntUI decorated window** | **`org.jetbrains.jewel:jewel-int-ui-decorated-window:0.36.0-261.24374.151`** | — (new) |
| Decompose | **3.3.0** | 3.2.2 |
| kotlinx-coroutines | **1.9.0** | 1.8.1 |
| JNA | 5.15.0 | 5.15.0 (unchanged) |
| Gradle wrapper | **8.13** | 8.10 |
| JDK (toolchain) | **21** | 17 |

The Jewel version encodes the IntelliJ Platform build it was compiled against:
`0.36.0-261.24374.151` = Jewel 0.36.0 / IJP 261.24374.151.

---

## Screens included and excluded

| Screen | Jewel | Material |
|---|---|---|
| MeetingList sidebar | ✅ `JewelMeetingListScreen` | ✅ (existing `Sidebar`) |
| Settings | ✅ `JewelSettingsScreen` | ✅ (existing `SettingsScreen`) |
| MeetingDetail | ⬜ `JewelPlaceholder` — markdown-renderer Protocol view not ported | ✅ |
| NewRecording | ⬜ `JewelPlaceholder` | ✅ |
| GenerateProtocol | ⬜ `JewelPlaceholder` | ✅ |
| Diagnostics | ⬜ `JewelPlaceholder` | ✅ |

**Reason for exclusions (D7):** The Protocol-detail screen requires the
`multiplatform-markdown-renderer-m3` library which has a hard dependency on Material 3
theming internals. Integrating it into a Jewel context would require a custom renderer
and is out of scope for a look-and-feel evaluation.

---

## Toggle behaviour

### Variant toggle

A `VariantToggleBar` is always visible at the top of the window (above the main content,
below the title bar in Jewel mode). It uses only Compose Foundation primitives — no
Material or Jewel theme — so it works identically in both variants.

- **Default on launch: Jewel.**
- Clicking `Material` or `Jewel` switches the variant live.
- Switching between `Jewel` (with `USE_DECORATED_WINDOW = true`) and `Material` triggers
  an **in-process window recreation** (the old AWT window closes and a new one opens in
  the same JVM process). There is a brief visual flash; **no JVM restart** is required.
  Navigation state resets on recreation — this is intentional and harmless for evaluation.

### Data-state toggle

The bar also has a `{Populated | Empty | Loading | Error}` control. Clicking a state:

1. Updates `prototype.fakes.currentDataState` (global mutable var in `SampleData.kt`).
2. Calls `root.meetingListViewModel.refresh()` to trigger a re-load from the fake repository.

This drives the `MeetingListState` state machine so the owner can verify empty / loading /
error renderings in **both** Jewel and Material variants.

> Note: `Loading` state keeps the coroutine permanently suspended (simulating a slow
> network). Switching away from `Loading` launches a new coroutine; the old one is
> cancelled only when the ViewModel is destroyed.

---

## `USE_DECORATED_WINDOW` — macOS fallback

Constant in `prototype/PrototypeRoot.kt`:

```kotlin
const val USE_DECORATED_WINDOW = true
```

| Value | Effect |
|---|---|
| `true` (default) | Jewel variant uses `DecoratedWindow` + `TitleBar` (Jewel-styled chrome with OS-native traffic-light buttons on macOS and Linux/Windows-style controls elsewhere) |
| `false` | Jewel variant falls back to a standard Compose `Window` hosting the same Jewel content — no decorated chrome, OS-native title bar |

**JBR requirement:** `DecoratedWindow` only works on **JetBrains Runtime (JBR)**. On standard
JVMs (e.g. Temurin, OpenJDK) it throws `IllegalStateException` at runtime. The prototype
detects this automatically via `isJBR()` in `PrototypeRoot.kt` and silently falls back to
`Window` — the Jewel theme and all content remain intact, only the custom window chrome is
absent.

To see the full decorated-window experience, run with JBR 21:

```bash
# Download JBR 21 via sdkman or mise, then:
JAVA_HOME=/path/to/jbr-21 ./gradlew :desktopApp:run
```

**When to set `false`:** If the decorated window misbehaves on your Mac (missing/misplaced
traffic-light buttons, incorrect insets under macOS Sequoia, etc.), flip this constant to
`false`, rebuild, and re-run. The Jewel content is unchanged; only the window chrome reverts
to the OS default.

### Single-window anti-flicker alternative

The current implementation uses `key(useDecoratedWindow)` to force Compose to recreate
the window composable when the variant crosses the `DecoratedWindow`↔`Window` boundary.
This causes a brief visible flash.

**Alternative approach (not implemented, documented here for reference):**
Always host content in a single `DecoratedWindow` and conditionally suppress the `TitleBar`
composable when `variant == Material`. This avoids any flash but means the Material variant
always runs inside Jewel's decorated window chrome. To try it: remove the `key()` +
if/else split, always use `DecoratedWindow`, and wrap `TitleBar(...)` in
`if (variant == UiVariant.Jewel)`.

---

## §B4 fallback status

**NOT taken.** The prototype uses the real Jewel IntUI theme via
`org.jetbrains.jewel.intui.standalone.theme.IntUiTheme`. No custom-theme fallback was
needed. The comparison is genuine **Jewel IntUI vs Material 3**, not a custom theme
approximation.

---

## Jewel-vs-Material control substitutions

The following Material 3 components have no direct Jewel equivalent and were replaced:

| Material 3 | Jewel replacement | Notes |
|---|---|---|
| `Switch` | `CheckboxRow` | Jewel has no toggle switch; checkbox is the nearest desktop-native control |
| `SnackbarHost` / `Snackbar` | Inline status text row | Jewel `DefaultBanner`/`InlineBanner` are full-width page banners, not toasts; a coloured text line near the Save button is the closest fit |
| `ExposedDropdownMenuBox` | `Dropdown` | Jewel `Dropdown` has a different slot API (header composable + `menuContent` DSL) |
| `TabRow` / `Tab` | `SegmentedControl` | Used for audio-source and transcription-language pickers |

`SegmentedControl` in Jewel uses a `buttons: List<SegmentedControlButtonData>` API rather
than individual `@Composable` children.

`IconButton` in Jewel passes a `ButtonState` parameter to its content lambda:
`IconButton(onClick = { ... }) { _ -> Icon(...) }`.

---

## Verdict template (for Section 06)

After evaluating both variants, fill in:

```
MeetingList — verdict: still mobile — notes:
Названия встреч совпадают с фоном, что затрудняет чтение.
Settings    — verdict: still mobile — notes:
Окно выглядит очень не нативно, все очень криво.
Overall stack decision: migrate to Qt
```

**Evaluated:** 2026-05-18. §B4 fallback NOT taken — comparison is genuine Jewel IntUI vs Material 3.
