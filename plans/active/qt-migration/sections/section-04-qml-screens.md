# Section 04 — QML screens: behavior port from frozen Compose

## Background
The actual UI screens, in QML/Fusion, driven only through the sidecar HTTP API.
The Compose app `ui-compose/` is **FROZEN** and is the reference for
**behavior / flows / domain wiring ONLY — never visual design** (the owner
explicitly dislikes the Compose design; copying it would defeat the migration).
Final visual design is a separate later workstream; this section ships plain
Fusion + sane layout.

## Requirements
Every Compose flow has a working QML equivalent driven through the 7 sidecar
routes; all data-states render; a full record→transcribe→generate-protocol
round-trip works against the real core; no Compose ViewModel behavior is
silently dropped.

## Dependencies
- Requires: section-03 (shell + ApiClient + JobPoller).
- Blocks: nothing.

## Implementation details
1. **Screen/flow inventory** from `ui-compose/` (behavior, NOT styling):
   MeetingList, MeetingDetail/Protocol, NewRecording, GenerateProtocol,
   Settings, Diagnostics. For each capture: data shown, state machine
   (populated/empty/loading/error), navigation edges, backing route(s) among the
   7 (transcribe; jobs submit/status; protocols; recordings start/stop;
   meetings list).
2. **ViewModel audit (critical — prevents dropped behavior).** Flow/business
   logic currently lives in Kotlin `shared/commonMain` ViewModels
   (`MeetingListVM`, `RecordingVM`, `ProtocolGenerateVM`), NOT in the Rust API.
   Audit each; for every piece of logic decide: move into the Rust core/API
   (**preferred** — keeps the Qt client thin) or reimplement in the Qt client.
   Produce an explicit VM→destination mapping table BEFORE writing screens.
3. **Implement each screen** in QML with Fusion controls via `ApiClient`:
   - MeetingList → `GET /api/v1/meetings`; empty/loading/error states.
   - NewRecording → `POST /api/v1/recordings` (name, source incl. system/mixed,
     echo_cancel) / `POST /api/v1/recordings/:id/stop`.
   - Transcription → `POST /api/v1/jobs` + `JobPoller` on
     `GET /api/v1/jobs/:id`.
   - Detail/Protocol → meeting data + `POST /api/v1/protocols`. **Markdown
     rendering is a first-class deliverable** (the protocol is the app's core
     output; the old Compose Material markdown lib does NOT port). Baseline: Qt
     `TextEdit` `textFormat: TextEdit.MarkdownText`; evaluate it against real
     generated protocols (headings, lists, tables, code) before anything
     heavier; document chosen path + limitations.
   - Settings → existing settings keys persisted by the core.
   - Diagnostics → log/health surface.
4. **Navigation** via Qt Quick `StackView`; reasonable state survival across
   sidecar restart.
5. **No bespoke control restyling** — plain Fusion + sane spacing only.

## Acceptance criteria
- [ ] VM→destination mapping table exists; every VM behavior is accounted for
      (moved or reimplemented), none dropped.
- [ ] Every Compose flow has a working QML equivalent driven only via the
      sidecar API.
- [ ] All four data-states render for the meeting list.
- [ ] Full record→transcribe→generate-protocol round-trip works on the real
      core.
- [ ] Protocol markdown view correctly renders headings/lists/tables/code from
      a real generated protocol; chosen render path + limits documented.
- [ ] No Material/Universal styling; Fusion only.

## Files to create/modify
- Create `qt-app/qml/screens/*.qml`, supporting C++/QML view glue.
- Read-only reference: `ui-compose/` (behavior only; do NOT edit it).
