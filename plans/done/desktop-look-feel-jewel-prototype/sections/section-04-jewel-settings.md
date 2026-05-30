# Section 04: Jewel Settings Screen (Form-Density Stress Point)

## Background

**Meeting Assistant** is a Rust + Kotlin Multiplatform Compose Desktop app
(`/ui-compose`). This **throwaway, never-merged** prototype re-renders two
screens with JetBrains Jewel and toggles them against the existing Material 3
screens so the owner can decide "desktop vs mobile". Sections 01–03 upgraded
the toolchain (CMP 1.10 / Kotlin 2.1.x + Jewel), built fakes + dev entry, and
delivered the shared `JewelTheme`/`JewelAppShell` + `JewelMeetingListScreen`.

This section builds the **Jewel rendering of the Settings screen** — the
**form-density stress point** and the hardest test of "desktop vs mobile",
because a dense form is exactly where Material 3 reads as mobile. Fidelity
rule (**D6**): mirror **content & density, not pixels** — every control the
Material screen has, with desktop-idiomatic sizing. All labels are Russian,
copied **verbatim** from the Material screen.

### What the Material Settings screen has (mirror it section-for-section)

`ui-compose/shared/src/commonMain/kotlin/ui/screens/SettingsScreen.kt`
(~lines 27–429): a `Scaffold` + `SnackbarHost`, a `SettingsToolbar`
(TopAppBar, title **"Настройки"**, back arrow), and a `SettingsForm` with:

1. **Anthropic API** — `OutlinedTextField` labelled **"API Key"**, placeholder
   `sk-ant-…`, a password show/hide trailing icon, validation supporting text
   (error if non-blank and not starting with `sk-ant-`).
2. **Recording** — segmented control **"Источник звука"**
   {**Микрофон**, **Система**, **Оба**}; a switch **"Подавление эха"**.
3. **Protocol template** (conditional, only if templates exist) — dropdown:
   **"По умолчанию"** + each template name.
4. **Transcription** — language segmented {**Русский**, **English**, **Авто**};
   **"Точность распознавания"** slider 1–5 (3 steps) with explanatory text;
   **"CPU потоки"** slider 0–16 (15 steps), `0` shown as **"Авто"**.
5. **Storage** — two path fields (`OutlinedTextField`, placeholder
   `/home/user/…`, error if non-blank and not starting with `/` or `~`):
   meetings dir + db path; a **model** dropdown showing
   `"name  ·  sizeMb MB"` + description per model with a **"Другой путь…"**
   option that reveals a custom path field; a prompts-dir path field.
6. **Save** — a full-width button **"Сохранить"**,
   `enabled = pathsValid && apiKeyValid`, with snackbar feedback on save.

`Settings` = `{ paths{model,db,meetingsDir,prompts}, anthropicApiKey,
recording{source,echoCancel}, defaultTemplate,
transcriber{language,beamSize 1–5,nThreads 0=auto} }`. The screen loads
`settings.get()`, `templatesList()`, `modelsList()` in a `LaunchedEffect`
into local state.

## Requirements

When this section is complete:

- `JewelSettingsScreen` mirrors **every** Settings section/control above with
  Jewel components, verbatim Russian labels, the same validation rules and the
  same enabled-rule for Save.
- It loads fixtures via the **same repository calls** the Material screen uses
  (`settings.get()`, `templatesList()`, `modelsList()`), via the fakes.
- It compiles and renders fully from the section-02 fakes; rendered via the
  shared `JewelTheme` (Cyrillic correct).

## Dependencies

- **Requires:** section 02 (fakes/dev entry) and section 03 (`JewelTheme`,
  `JewelAppShell` — Settings is rendered inside the shell). Section 01 gate
  passed.
- **Blocks:** section 05 (toggle/window/README needs both Jewel screens).
- **Parallelizable with:** section 03 (different files) — but uses section
  03's `JewelTheme`/shell, so coordinate if run truly in parallel (the shell's
  content `when` must route the Settings `Screen` to `JewelSettingsScreen`).

## Implementation Details

> **API-shape caveat:** Jewel component names/signatures (`TextField`,
> radio/segmented, `Checkbox`/toggle, `Dropdown`/combo, sliders,
> `DefaultButton`/`OutlinedButton`, inline banners) differ by Jewel version.
> Resolve exact APIs from the **pinned Jewel version's official standalone
> samples/docs**. Where Jewel lacks an exact equivalent, use the closest
> Jewel primitive and note the substitution in `PROTOTYPE.md` (section 05).

### Step 1 — Load state (mirror the Material screen's shape)

In `JewelSettingsScreen`, in a `LaunchedEffect`, call the same repository
methods the Material screen calls — `root.settings.get()`,
`root.settings.templatesList()`, `root.settings.modelsList()` (exact names per
`SettingsRepository` / `RootComponent`) — and hold them in local Compose
state mirroring the Material screen's local-state shape so behavior matches.
Replicate the derived validation:

- `apiKeyValid` = api key blank, unchanged, or starts with `sk-ant-`.
- `pathsValid` = every path blank or starts with `/` or `~`.

### Step 2 — Sections, in order, verbatim Russian labels

Render with Jewel components, desktop density (let Jewel Int UI defaults set
sizing — compact, ~28–32.dp controls; do not inflate to Material touch sizes):

1. **Anthropic API:** Jewel text field labelled **"API Key"**, placeholder
   `sk-ant-…`, a secret/password mode with a show/hide affordance, and
   validation/supporting text (error styling if invalid).
2. **Recording:** **"Источник звука"** as a Jewel segmented/radio-chain
   {**Микрофон**, **Система**, **Оба**}; **"Подавление эха"** as a Jewel
   checkbox/toggle.
3. **Protocol template** (only if templates non-empty): a Jewel dropdown with
   **"По умолчанию"** + each template name.
4. **Transcription:** language Jewel segmented/radio {**Русский**,
   **English**, **Авто**}; **"Точность распознавания"** Jewel slider 1–5 with
   the explanatory text; **"CPU потоки"** Jewel slider 0–16, displaying
   **"Авто"** when value is 0.
5. **Storage:** two Jewel path fields (placeholder `/home/user/…`, error if
   non-blank & not `/`/`~`) for meetings dir + db; a Jewel **model** dropdown
   listing `"name  ·  sizeMb MB"` + description per model with a **"Другой
   путь…"** option revealing a custom path field; a prompts-dir path field.
6. **Save:** a Jewel button **"Сохранить"**,
   `enabled = pathsValid && apiKeyValid`; on click build the updated
   `Settings` and call the same save method the Material screen uses; show
   Jewel feedback (a Jewel inline status/banner — Jewel has no Material
   `Snackbar`; use the closest Jewel notification primitive or an inline
   status line, and note the substitution).

### Step 3 — Toolbar + routing

- A Jewel header **"Настройки"** with a back affordance →
  `root.onBackToList()` (exact nav method per `RootComponent.kt`).
- Ensure section 03's `JewelAppShell` content `when` routes the Settings
  `Screen` case to `JewelSettingsScreen` so navigation from the
  MeetingList footer ("Настройки") reaches it.

Reuse `shared` domain types by import. Use only the public repository/nav
surface. **Do not edit `shared`.**

## Acceptance Criteria

- [ ] `JewelSettingsScreen` loads `settings.get()` / `templatesList()` /
      `modelsList()` (same calls as the Material screen) via the fakes in a
      `LaunchedEffect`; local-state shape mirrors the Material screen.
- [ ] All six sections present with **verbatim Russian labels**: API key
      (secret + show/hide + validation), recording source segmented + echo
      toggle, protocol template dropdown, language segmented + accuracy slider
      1–5 + threads slider 0–16 ("Авто" at 0), storage path fields with
      `/`/`~` validation + model dropdown (`"name · sizeMb MB"` + desc) +
      "Другой путь…" custom path + prompts dir, Save button.
- [ ] Save is `enabled = pathsValid && apiKeyValid` with the same validation
      rules; clicking it calls the same save method and shows Jewel feedback.
- [ ] Toolbar "Настройки" + back → `root.onBackToList()`; navigation from the
      MeetingList footer reaches this screen via `JewelAppShell`.
- [ ] Renders inside the shared `JewelTheme`; **Cyrillic correct** (no tofu).
- [ ] Any Jewel-vs-Material control substitution is noted for `PROTOTYPE.md`.
- [ ] Compiles and renders fully from section-02 fakes; no edits to `shared`
      or the Rust core; code isolated under `prototype/jewel/`.

## Files to Create/Modify

**Create:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelSettingsScreen.kt`

**Modify:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelAppShell.kt`
  — route the Settings `Screen` case to `JewelSettingsScreen`.
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelComponents.kt`
  — only if shared form helpers (section header, labelled field) are factored
  out.

**Do not modify:** `shared` (including `SettingsScreen.kt`, ViewModels,
domain), the Rust core.
