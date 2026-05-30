# Section 03: Jewel MeetingList Screen + Jewel Theme/Shell

## Background

**Meeting Assistant** is a Rust + Kotlin Multiplatform Compose Desktop app
(`/ui-compose`). This is a **throwaway, never-merged** prototype to decide
whether the app's "mobile feel" is a theming choice (fixable) or a stack
limitation (justifies Qt). Sections 01–02 upgraded the toolchain to
CMP 1.10 / Kotlin 2.1.x with real Jewel, and stood up a dev entry that runs
the **existing Material screens** on fake data with no FFI.

This section builds the **Jewel rendering of the MeetingList screen**, plus
the shared Jewel theme and app shell that section 04's Settings screen also
uses. Fidelity rule (**D6**): mirror **content & density, not pixels** — same
data, same controls, same sections, but desktop-idiomatic sizing/spacing
(Jewel's Int UI gives ~28–32.dp controls, ~6–8.dp spacing, ~13–14.sp text for
free). All labels are Russian and copied **verbatim** from the Material
screens.

### What the Material MeetingList looks like (the thing to mirror)

The "MeetingList" is really the **Sidebar**
(`ui-compose/shared/src/commonMain/kotlin/ui/Sidebar.kt`, ~lines 24–93):

- A header with title **"Встречи"** and add (`Icons.Default.Add`) + refresh
  `IconButton`s.
- A `LazyColumn` of `MeetingListItem` rows. Each row shows: meeting **name**
  (body text), **`formatDate(createdAt)`** (small label), and conditional
  status chips **"Транскрипт"** (if `hasTranscript`) / **"Протокол"** (if
  `hasProtocol`). The selected row has a highlighted background.
- Empty state: centered **"Нет встреч"**.
- Footer: Settings + Diagnostics icon buttons.

State is `MeetingListState { Loading | Success(List<Meeting>) | Error(msg) }`
exposed by `root.meetingListViewModel.state`. `Meeting` =
`{ id, name, audioPath, hasTranscript, hasProtocol, createdAt: Long }`. The
overall app layout (`AppContent`) is a `Row`: sidebar (~280.dp) |
`VerticalDivider` | content pane that switches on the sealed `Screen` value
via Decompose `subscribeAsState()`.

## Requirements

When this section is complete:

- A dark Jewel theme wrapper exists, renders Cyrillic correctly, and does
  **not** nest a Material `MaterialTheme`.
- A `JewelAppShell` mirrors `AppContent`'s sidebar | divider | content layout
  and switches content on `root.screen`.
- `JewelMeetingListScreen` faithfully mirrors the Sidebar's content & density
  (header + add/refresh, list rows with name/date/status chips,
  selection/hover, empty/loading/error, footer nav) using Jewel components and
  the **same VM/nav callbacks** the Material Sidebar uses.
- It compiles and renders from the fake data set up in section 02.

## Dependencies

- **Requires:** section 02 (fakes, dev entry, `PrototypeRoot`, Material
  variant running). Section 01 gate passed.
- **Blocks:** section 05 (toggle/window/README needs both Jewel screens).
- **Parallelizable with:** section 04 (Jewel Settings) — both depend only on
  02, write different files.

## Implementation Details

> **API-shape caveat:** all Jewel APIs (`IntUiTheme`, theme providers,
> component names, `LazyColumn`/list, icon buttons) differ by Jewel version.
> Resolve exact names/signatures from the **pinned Jewel version's official
> standalone samples/docs**, not from memory or the plan's pseudocode. Use the
> closest Jewel primitive where an exact equivalent is missing and note it.

### Step 1 — `JewelTheme.kt` (shared by sections 03 & 04)

`ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelTheme.kt`:

- A composable that wraps content in Jewel's **dark** `IntUiTheme`
  (`org.jetbrains.jewel.intui.standalone.theme.*`). Match the app's dark
  default so the Material↔Jewel comparison is fair.
- **Do NOT** nest a Material `MaterialTheme` inside it — Jewel and Material
  must live in **separate composable subtrees** (nested Material values win
  and break Jewel styling). The Jewel screen set is wholly Jewel.
- Expose any shared spacing/typography tokens the two Jewel screens need.
- **Cyrillic check (Phase-2 exit requirement):** every label is Russian. Run
  the screen and confirm Jewel's default Int UI font renders Cyrillic. If it
  shows tofu/boxes, supply a Cyrillic-capable `FontFamily` (bundled
  Inter/Noto, or a system font) to the Jewel `TextStyle` / `ThemeDefinition`.
  A font bug here would invalidate the owner's verdict — treat correct
  Cyrillic as mandatory before this section is "done". (Section 04 reuses this
  theme; fixing it here fixes it for Settings too.)

### Step 2 — `JewelAppShell.kt`

`ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelAppShell.kt`:

- A `Row` mirroring `AppContent`'s layout: Jewel sidebar (~280.dp) | a
  divider | a content pane (weight 1).
- Subscribe to `root.screen` via Decompose `subscribeAsState()` exactly like
  `AppContent` does, and `when` over the sealed `Screen` value to render
  `JewelMeetingListScreen` / `JewelSettingsScreen` (section 04).
- For any other `Screen` value (detail, recording, diagnostics, etc.) render a
  simple Jewel placeholder ("Not in prototype scope") so navigation never
  crashes.
- The sidebar pane hosts `JewelMeetingListScreen` (the list *is* the
  sidebar), matching the Material structure.

### Step 3 — `JewelMeetingListScreen.kt`

`ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelMeetingListScreen.kt`:

- Consume `root.meetingListViewModel.state` (the same `MeetingListState`
  StateFlow the Material Sidebar uses — collect as Compose state).
- **Header row:** title **"Встречи"** + Jewel icon buttons for **add** and
  **refresh**, wired to the **same callbacks** the Material Sidebar uses
  (the add → new-meeting action; refresh → `meetingListViewModel.refresh()`
  or whichever method the real Sidebar calls — confirm from `Sidebar.kt`).
- **`Success`:** a Jewel `LazyColumn` of rows. Each row: meeting **name**,
  **`formatDate(createdAt)`** (reuse the existing `formatDate` helper from
  `shared` — do not reimplement), conditional **"Транскрипт"** /
  **"Протокол"** status chips. Selected/hovered row gets Jewel
  selection/hover styling. Click → `root.onMeetingSelected(meeting)` (exact
  nav method per `RootComponent.kt`).
- **`Loading`:** Jewel progress/spinner. **`Error`:** the error message.
- **Empty (`Success` with empty list):** centered **"Нет встреч"**.
- **Footer:** Jewel icon buttons → `root.onSettingsRequested()` (and the
  diagnostics nav method, matching the Material footer).
- Density: compact desktop sizing — let Jewel Int UI defaults drive it; don't
  re-inflate to Material touch sizes.

Reuse `shared` helpers (`formatDate`, domain types) by import — **do not edit
`shared`**. Use only the public VM/nav surface of `RootComponent`.

### Step 4 — Wire into `PrototypeRoot`

Replace the section-02 Jewel placeholder so the Jewel branch of
`PrototypeRoot` renders `JewelTheme { JewelAppShell(root) }`. (The full
toggle/decorated-window wiring is section 05; here just make the Jewel branch
show real Jewel content for verification.)

## Acceptance Criteria

- [ ] `JewelTheme` wraps content in Jewel's **dark** `IntUiTheme`; no nested
      Material `MaterialTheme`.
- [ ] **Cyrillic renders correctly** in the Jewel variant (no tofu) — a
      Cyrillic-capable font is supplied if Jewel's default lacks glyphs.
- [ ] `JewelAppShell` mirrors `AppContent`'s sidebar|divider|content layout
      and switches on `root.screen` via Decompose `subscribeAsState()`; other
      screens show a safe placeholder (no crash on navigation).
- [ ] `JewelMeetingListScreen` mirrors the Sidebar's content & density:
      "Встречи" header + add/refresh, list rows with name +
      `formatDate(createdAt)` + "Транскрипт"/"Протокол" chips +
      selection/hover, "Нет встреч" empty state, Loading + Error states,
      footer nav.
- [ ] All actions use the **same VM/nav callbacks** as the Material Sidebar
      (`onMeetingSelected`, `onSettingsRequested`, refresh, …); `formatDate`
      and domain types reused from `shared` by import.
- [ ] Compiles; renders from the section-02 fakes; the Jewel branch of
      `PrototypeRoot` shows `JewelTheme { JewelAppShell(root) }`.
- [ ] No edits to `shared` ViewModels/domain or the Rust core; code isolated
      under `prototype/jewel/`.

## Files to Create/Modify

**Create:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelTheme.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelAppShell.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelMeetingListScreen.kt`
- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/jewel/JewelComponents.kt`
  (optional — small shared helpers like a section header / status chip)

**Modify:**

- `ui-compose/desktopApp/src/desktopMain/kotlin/prototype/PrototypeRoot.kt` —
  Jewel branch renders `JewelTheme { JewelAppShell(root) }` instead of the
  placeholder.

**Do not modify:** `shared` (including `Sidebar.kt`, `AppContent.kt`,
ViewModels, domain), the Rust core.
