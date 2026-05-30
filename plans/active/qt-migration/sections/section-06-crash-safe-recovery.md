# Section 06 — Crash-safe recording recovery

## Background
The sidecar isolates a core crash from the GUI (Q2/Q8). But without recording
recovery that isolation is cosmetic: codebase fact —
`adapters/src/audio/cpal_capture.rs` streams samples to disk incrementally via
`hound::WavWriter`+`BufWriter`, and the WAV is only validly **finalized on a
clean `stop`** (`.finalize()` patches the RIFF/data-chunk lengths). A core
process kill mid-recording leaves audio bytes on disk but an unpatched header →
the file is not playable, and (if the crash was before `meeting_repo.save`) it
may have no DB row. v1 fixes this with a startup recovery pass (the
crash-friendly format is explicitly fast-follow, not v1).

## Requirements
On core startup the sidecar finds orphaned in-progress recordings, makes their
WAV files valid and playable, and ensures a corresponding `meetings` row exists
so the recording is visible and transcribable. The pass is idempotent.

## Dependencies
- Requires: section-02 (runs in the `meeting-server` boot path).
- Blocks: nothing. Parallelizable with sections 03–04.

## Implementation details
1. **Scan.** On core startup, walk `meetings_dir` for `*/recording.wav`. The
   directory slug is `<YYYY-MM-DD_HH-MM_uuid8>`. An entry is an **orphan** if:
   (a) no `meetings` row references its `audio_path`, OR (b) a row exists but the
   WAV header is unfinalized (data-chunk size 0 / inconsistent with file size).
2. **WAV-header reconstruction — do NOT assume a 44-byte header.** `hound`
   writes float WAV as `WAVE_FORMAT_IEEE_FLOAT`, which conventionally includes a
   `fmt ` (18+ byte) chunk and a `fact` chunk; the header is not 44 bytes.
   Algorithm: parse the RIFF chunk list from offset 12, walking
   `ckID`/`ckSize` until the `data` chunk; record its byte offset `data_off`.
   True payload length = `file_size - data_off`, truncated down to a whole
   sample frame (`channels * 4` bytes for f32). Patch the `data` chunk `ckSize`
   and the top-level RIFF `ckSize` (`file_size - 8` after truncation) in place.
   **Verify the parser against a real `hound` f32 file produced by this
   codebase before implementing** (unit test: record → truncate →
   reconstruct → assert playable + expected sample count).
3. **DB reconciliation.** If no `meetings` row exists, recreate one from the
   slug (`<YYYY-MM-DD_HH-MM_uuid8>` → timestamp + uuid8 → `Meeting` with
   `audio_path`) so the recording appears in the list and can be transcribed.
4. **Idempotency.** Running the pass twice is a no-op (already-valid files and
   existing rows are skipped).

## Acceptance criteria
- [ ] Kill the sidecar mid-recording → on next start the WAV is finalized
      (valid, playable, correct frame count).
- [ ] The recovered recording appears in `GET /api/v1/meetings` and can be
      transcribed.
- [ ] Both orphan kinds handled: no DB row, and unfinalized header with a row.
- [ ] Running startup recovery twice changes nothing (idempotent).
- [ ] The RIFF parser is unit-tested against a real codebase-produced hound f32
      WAV (NOT a hard-coded 44-byte assumption).

## Files to create/modify
- Create a recovery module (e.g. `rust/crates/adapters/src/audio/recovery.rs`
  or a core usecase) called from the `meeting-server` boot sequence
  (section-02).
- Add unit tests for the WAV parser/reconstruction.
