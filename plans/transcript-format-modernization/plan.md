# Transcript format modernization

## Problem

`transcript.md` is written as one flat blob of text ([worker.rs](../../rust/crates/adapters/src/worker.rs) passes only `transcript.text` to `write_transcript`). The Whisper adapter already computes per-segment timestamps (`Vec<Segment>` with `start_ms`/`end_ms`), but they are discarded at the write step. The legacy Python output had a header (title + date) and `[MM:SS]` timestamped lines; we want to restore and modernize that.

## Scope

**In:**
- A pure domain renderer in `meeting-core` turning `Transcript` + metadata → Markdown.
- Header `# Транскрипция: <name>` + `**Дата:** DD.MM.YYYY`.
- `[MM:SS]` per segment, rolling to `[HH:MM:SS]` past one hour.
- Blank-line pause marker when the gap between consecutive segments exceeds 3s.
- Wire the renderer into the worker's single transcript write site; the **file** gets the rendered Markdown, the **DB** keeps clean prose (`transcript.text`) for the LLM.

**Out:**
- VAD / inference-time silence skipping — stays deferred (RULES.md "VAD — not implemented").
- Persisting structured segments (`transcript.json`) for re-render / seek-to-timestamp — deferred; re-render still requires re-running Whisper.
- Speaker diarization.

## Deliverables

- **New** `rust/crates/core/src/usecases/transcript_render.rs`
  - `pub struct TranscriptMeta<'a> { title: &'a str, date: &'a str }`
  - `pub fn render_markdown(t: &Transcript, meta: &TranscriptMeta) -> String`
  - `fn fmt_ts(ms) -> String` (MM:SS / HH:MM:SS), `PAUSE_GAP_MS = 3000`.
  - Unit tests: empty segments (header only), single segment, HH:MM:SS rollover, pause-gap insertion, trims segment text, header formatting.
- **Edit** `rust/crates/core/src/usecases/mod.rs` — register module + re-export.
- **Edit** `rust/crates/adapters/src/worker.rs::run_transcribe` — build `TranscriptMeta` from `meeting.name` + `meeting.created_at` (format unix → `DD.MM.YYYY`), render, pass rendered Markdown to `write_transcript`, keep `transcript.text` for `save_transcript_file`/`save_transcript`.

### Test plan
- `cargo test -p meeting-core transcript_render`
- `cargo test --manifest-path rust/Cargo.toml` (workspace regression)

## Decisions (inline ADRs)

1. **Render from segments, in a pure core function** (not in the file-store adapter): keeps `MeetingFileStore::write_transcript(&str)` dumb, makes formatting unit-testable, leaves the port unchanged.
2. **File = rich timestamped view; DB `transcript_text` = clean prose.** The LLM (`generate_protocol`) consumes the DB text; timestamps would add noise/tokens. The human file is the rendered artifact.
3. **Pause = blank line, not a `[пауза Ns]` label.** Lowest visual noise, matches the legacy look; threshold 3s.
4. **No segment persistence (yet).** Re-render needs re-running Whisper. Revisit only if seek-to-timestamp lands.
