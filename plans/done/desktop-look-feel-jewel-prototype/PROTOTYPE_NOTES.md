# Prototype Branch: proto/jewel-look-feel

**Status:** Section 01 — Toolchain + Jewel Gate  
**Purpose:** Throwaway experiment to evaluate JetBrains Jewel vs Material 3 for the desktop UI. **Never merge.**

---

## Final Resolved Versions

| Artifact | Planned | Actual Resolved |
|---|---|---|
| Kotlin | 2.1.20 | **2.3.10** (Jewel 0.36 requires Kotlin 2.3.x) |
| Compose Multiplatform | 1.10.0 | **1.10.0** |
| Compose compiler plugin | = Kotlin | **2.3.10** |
| Jewel standalone | 0.36 | **0.36.0-261.24374.151** (Maven Central, IJP 261) |
| Jewel decorated-window | 0.36 | **0.36.0-261.24374.151** |
| Decompose | 3.3.x | **3.3.0** |
| kotlinx-coroutines | 1.9.0 | **1.9.0** |
| Gradle wrapper | 8.13 | **8.13** |
| JNA | 5.15.0 | **5.17.0** (forced up by Jewel transitive) |
| JDK | 17 | **21** (temurin-21.0.7+6 via mise) |

### JDK Version Deviation from Plan

Jewel 0.36 class files are compiled at class file version 65.0 (Java 21). The project used JDK 17 (class file version 61.0), which throws `UnsupportedClassVersionError` at runtime. `.mise.toml` on this proto branch pins JDK to `temurin-21.0.7+6.0.LTS`. The non-proto branches still use JDK 17.

### Kotlin Version Deviation from Plan

The plan specified Kotlin 2.1.20. The actual Jewel 0.36.0-261.24374.151 artifact was compiled with Kotlin 2.3.x (binary metadata version `2.3.0` in its class files). Using Kotlin 2.1.x would fail with "The binary version of its metadata is 2.3.0, expected version is 2.1.0". Upgraded to Kotlin **2.3.10** (latest stable 2.3.x). CMP 1.10.0 is compatible with Kotlin 2.3.x.

### Jewel Artifact Location

Jewel 0.28+ is published on **Maven Central** (not kpm/public). The kpm/public repo only hosts Jewel ≤ 0.27. Both repos are listed in `settings.gradle.kts` for completeness; new Jewel only resolves from Maven Central.

Exact coordinate: `org.jetbrains.jewel:jewel-int-ui-standalone:0.36.0-261.24374.151`

### DecoratedWindow API

In Jewel 0.36 standalone, `DecoratedWindow`/`TitleBar` moved to the IntelliJ IDE plugin context (`intellij.platform.jewel.decoratedWindow`). The `jewel-int-ui-decorated-window` artifact for this version is an IDE plugin module containing no standalone composable classes. For standalone (non-IDE) desktop use, the correct API is:
- `org.jetbrains.jewel.intui.standalone.window.Window` — thin wrapper over Compose Desktop `Window` that provides `LocalComponent`
- `org.jetbrains.jewel.intui.standalone.theme.IntUiTheme` — theme wrapper

`PrototypeSmoke.kt` uses `Window + IntUiTheme(isDark = true)` — the verified standalone API.

### Resolution Strategy

A `resolutionStrategy.eachDependency` is applied in `desktopApp/build.gradle.kts` to force core CMP transitive deps (from Decompose `extensions-compose`) to 1.10.0. Material3 is excluded from this forcing since CMP 1.10.0's `compose.material3` resolves to version 1.9.0 (not 1.10.0).

### Non-Target Screen Changes (D7)

No screens were stubbed. The `multiplatform-markdown-renderer-m3:0.30.0` resolved and compiled without issues on CMP 1.10.0 + Kotlin 2.3.10.

### materialIconsExtended

`compose.materialIconsExtended` resolved on CMP 1.10.0 without changes. The Material baseline compiles.

### Compiler Warnings (Upgrade-Inherent)

The following deprecation warnings appear in `:shared:compileKotlinDesktop` — these are inherent to the CMP/Material3 API changes between 1.7.3 and 1.10.0. Not introduced by this branch; no action required for the prototype:
- `TabRow` → replaced by `PrimaryTabRow`/`SecondaryTabRow`
- `Icons.Filled.ArrowBack` → use `AutoMirrored` variant
- `MenuAnchorType` → renamed to `ExposedDropdownMenuAnchorType`
- `compose.foundation`, `compose.material3`, `compose.materialIconsExtended` → direct dependency API deprecated in KMP DSL

---

## Gate Status: PASSED ✓

Visual confirmation by owner:
- Jewel window opened ✓
- Russian label "Встречи — проверка кириллицы" rendered **without tofu** ✓
- Jewel-styled `DefaultButton` drew correctly ✓

**Light background note:** `IntUiTheme(isDark=true)` themes Jewel components but does not paint the root window background. Window appeared light (system default). In Section 02/03, use `Surface(color = JewelTheme.globalColors.paneBackground)` at the root to get full dark background.

`PrototypeSmoke.kt` deleted; `mainClass` reverted to `"MainKt"`. **Section 02 is unblocked.**
