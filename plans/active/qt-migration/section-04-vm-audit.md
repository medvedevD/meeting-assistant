# Section 04 — ViewModel Audit & Markdown Decision

Behavior reference: frozen `ui-compose/` (flows only, never visual design).
Boundary: the **7 sidecar routes** + unauthenticated `/health`, `/version` meta.
The Rust core/API contract is **frozen** at section-03's `PROTOCOL_VERSION`;
section-04 is QML-screens-only, so any "needs a new route" item is recorded as
a *reimplement-in-client* decision plus a flagged future core/API route — never
silently dropped.

## The 7 routes (+ meta)

| Route | Shape |
|---|---|
| `GET /api/v1/meetings` | → `[{id,name,audio_path,has_transcript,created_at}]` |
| `POST /api/v1/recordings` | `{name?,source,echo_cancel}` → `{id,name,audio_path,created_at}` |
| `POST /api/v1/recordings/:id/stop` | → `{id,name,audio_path,created_at}` |
| `POST /api/v1/transcribe` | `{path,meeting_id?}` → `{text,language,segments}` (sync; persists if `meeting_id`) |
| `POST /api/v1/jobs` | `{audio_path,name}` → `{id,meeting_id,status,attempts,…}` (async) |
| `GET /api/v1/jobs/:id` | → same `JobResponse` |
| `POST /api/v1/protocols` | `{transcript,template_name?,meeting_name?}` → `{markdown}` |
| `GET /health` `GET /version` | meta, no auth |

## VM → destination mapping

### MeetingListViewModel
| VM behavior | Destination | Notes |
|---|---|---|
| State machine `Loading/Success/Error` | **Qt client** (`MeetingStore`) | pure UI state |
| 4th state **Empty** (derived in Sidebar, not the VM) | **Qt client** | `success && meetings==0` → empty branch |
| `load()` on init + `refresh()` | **Qt client** | `GET /api/v1/meetings` |
| error → `e.message ?: "Unknown error"` | **Qt client** | `requestFailed` → error text |

No core change. Thin client.

### RecordingViewModel
| VM behavior | Destination | Notes |
|---|---|---|
| `start` guard "ignore unless Idle" | **Qt client** | state check before POST |
| name `ifBlank{"Встреча"}` | **Qt client** | default applied client-side (core also defaults, parity kept) |
| `start` → `RecordingRepository.start` | **core/API (already there)** | `POST /api/v1/recordings {name,source,echo_cancel}` |
| in-memory elapsed-seconds timer | **Qt client** | pure UI; QML `Timer`. Correctly a client concern |
| `stop` → `Stopping` → repo.stop → `Done` | **core/API (already there)** | `POST /api/v1/recordings/:id/stop` |
| `Done` → reset + refresh list + go to Generate(autoStart) | **Qt client** | navigation |
| `Error` state + "Попробовать снова" | **Qt client** | reset to idle |

No core change.

### ProtocolGenerateViewModel  *(the load-bearing one)*
| VM behavior | Destination | Notes |
|---|---|---|
| guard: run only from Idle/TranscriptFailed/Failed | **Qt client** | preserved |
| if `!hasTranscript`: submit job + **poll** until DONE/FAILED, tolerate transient poll errors, surface `attempts` | **Qt client, mechanism changed** | see decision ① |
| then generate protocol | **core/API (already there)** | `POST /api/v1/protocols` |
| state machine Idle→Transcribing→GeneratingProtocol→Done/Failed/TranscriptFailed | **Qt client** | preserved 1:1 |
| `Done` → refresh list + open Detail | **Qt client** | navigation |
| retry semantics from both failure states | **Qt client** | preserved |

**Decision ① — async job-poll → sync `/transcribe`.** `POST /api/v1/jobs` +
`GET /api/v1/jobs/:id` is async and, on completion, persists the transcript
*server-side* — but **no route returns transcript text**, and
`POST /api/v1/protocols` requires the text. So to feed `/protocols`, the client
calls **sync `POST /api/v1/transcribe {path,meeting_id}`** (returns the text and
persists it), then `/protocols`. Observable behavior is preserved (transcribe →
generate; progress shown; errors surfaced; retry works). What changes: the
`attempts` counter has no analogue on the sync route (generic progress text
instead) — a cosmetic loss, documented, not silent. PREFERRED long-term fix is
a core/API route (`POST /api/v1/protocols {meeting_id}` or
`GET /api/v1/meetings/:id` exposing the saved transcript); out of section-04
scope because the contract is frozen at 7 routes.

**`POST /api/v1/jobs` + `JobPoller` is still used** — for the **Import audio
file** flow (Compose `NewRecordingScreen.onImport` → `jobSubmit`): a background
transcription job tracked by `JobPoller` to completion, then list refresh. This
keeps the async path and the section-03 `JobPoller` exercised end-to-end.

### Cross-VM repository behaviors with no route (audited gaps)
| Compose behavior | Decision |
|---|---|
| `MeetingRepository.protocolLoad(id)` (view a previously generated protocol) | **Reimplement in client, scoped:** in-session generated markdown is cached in `MeetingStore` and rendered on Detail. Cross-restart persisted-protocol view is **not possible via 7 routes** → documented limitation; flagged future core/API route. |
| `Meeting.hasProtocol` | **Dropped from UI by necessity:** `GET /api/v1/meetings` (`MeetingItem`) does **not** include `has_protocol`. Sidebar shows only the "Транскрипт" badge. Flagged: add `has_protocol` to `MeetingItem`. |
| `SettingsRepository` get/set/templatesList/modelsList | **Reimplement in client (partial):** recording defaults (source, echo_cancel, default template) are real inputs to `/recordings`+`/protocols` → persisted client-side via QtCore `Settings`. Core-managed keys (model/db/prompts/API key/transcriber) have **no route** → Settings screen states this honestly; flagged future core/API route. |
| `DiagnosticsRepository` get/logs/openPath | **Reimplement in client (partial):** Diagnostics surfaces the **log/health surface** the section asks for — live `/health`, `/version` (build + protocol range), sidecar URL, enforced style. Device/path/ffmpeg/log enumeration & `openPath` have **no route** → stated honestly; flagged future core/API route. |

Nothing is silently dropped: every item above is either moved (already in
core/API), reimplemented in the client, or explicitly recorded as a frozen-
contract limitation with a named future route.

## Markdown rendering decision (first-class deliverable)

**Chosen:** Qt Quick `TextEdit { readOnly: true; textFormat:
TextEdit.MarkdownText; wrapMode: TextEdit.Wrap; selectByMouse: true }` inside a
`ScrollView`. No third-party lib (the Compose `com.mikepenz.markdown.m3` lib
does not port; the section mandates evaluating the built-in baseline first).

Qt's `MarkdownText` uses the bundled **md4c** parser (CommonMark + GFM
extensions). Verified against a real generated protocol (Russian, from
`POST /api/v1/protocols`):

| Element | Renders |
|---|---|
| `#`…`######` headings | ✅ scaled, bold |
| ordered / unordered / nested lists | ✅ |
| **bold**, *italic*, ~~strike~~, `inline code` | ✅ |
| fenced code blocks | ✅ monospace block (⚠ **no syntax highlighting** — md4c has no lexer; acceptable for meeting protocols) |
| GFM tables | ✅ rendered as a grid (⚠ minimal borders; Fusion default) |
| block quotes, links, thematic breaks | ✅ |

**Limitations (documented):** no code syntax highlighting; table styling is
plain (acceptable — protocols are prose + tables, not code); raw inline HTML is
escaped, not interpreted (desirable for LLM output safety). Heavier renderers
(WebEngine, custom md→QML) were not needed and were rejected as scope/footprint
overkill for v1. If protocols later require richer tables/highlighting that is a
later visual-design workstream item, not section-04.