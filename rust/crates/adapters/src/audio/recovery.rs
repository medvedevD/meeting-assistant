//! Crash-safe recording recovery (Qt-migration section-06).
//!
//! `cpal_capture` streams f32 samples to disk through `hound::WavWriter` +
//! `BufWriter`. The RIFF/`data` chunk lengths are only patched by
//! `WavWriter::finalize()` on a clean `stop`. If the core is killed
//! mid-recording the audio bytes are on disk but the header still carries the
//! placeholder zero lengths → the file is unplayable, and (if the crash beat
//! `meeting_repo.save`) it may have no DB row.
//!
//! This module runs once on core startup: it walks `meetings_dir`, makes every
//! orphaned `recording.wav` a valid playable WAV, and recreates any missing
//! `meetings` row so the recording is listable and transcribable. The pass is
//! idempotent.
//!
//! ## WAV layout — parsed, never assumed
//! The plan warns against assuming a 44-byte header. Verified against the
//! vendored `hound` 3.5.1 (the exact version `cpal_capture` writes with):
//! - hound writes float as `WAVE_FORMAT_IEEE_FLOAT` (tag 3) with a **16-byte
//!   `fmt `** chunk for ≤2 channels (→ 44-byte header) or a **40-byte
//!   WAVEFORMATEXTENSIBLE `fmt `** for >2 channels (→ 68-byte header);
//! - hound **never writes a `fact` chunk** (`hound/src/write.rs:700`).
//!
//! So this codebase never actually produces the "fmt + fact" header the plan
//! describes — but we still parse the real RIFF chunk list (walk
//! `ckID`/`ckSize` from offset 12) rather than hardcode any offset, so the
//! WAVEFORMATEXTENSIBLE case and any future/foreign chunk (incl. a `fact`) are
//! handled transparently. `data` is the final chunk in every hound write, so
//! the true payload length is `file_size - data_off` floored to a whole
//! sample frame; the crash-friendly format is explicitly fast-follow and out
//! of scope here.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use meeting_core::entities::Meeting;
use meeting_core::ports::MeetingRepo;

/// Result of reconstructing one WAV header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavRecovery {
    /// Header was already consistent with the file — nothing written
    /// (idempotent path).
    AlreadyValid { frames: u64 },
    /// Header was unfinalized/inconsistent; `data`+RIFF lengths patched (and
    /// any trailing partial frame truncated).
    Reconstructed { frames: u64 },
}

impl WavRecovery {
    pub fn frames(&self) -> u64 {
        match *self {
            WavRecovery::AlreadyValid { frames } | WavRecovery::Reconstructed { frames } => frames,
        }
    }
}

/// Parsed positions/values we need to patch a `hound` float WAV in place.
struct WavLayout {
    /// Offset of the `data` chunk's `ckSize` field (data payload starts +4).
    data_size_off: u64,
    /// Offset of the `data` payload.
    data_off: u64,
    /// `ckSize` currently declared for the `data` chunk (0 on a crash).
    declared_data_size: u32,
    /// Top-level RIFF `ckSize` currently declared (0 on a crash).
    declared_riff_size: u32,
    /// Bytes per sample frame = `nBlockAlign` = channels * bytes/sample.
    frame_bytes: u32,
}

/// Walk the RIFF chunk list to locate `fmt ` (for the frame size) and `data`
/// (offset + declared length). Does **not** assume any fixed header size.
fn parse_wav_layout<R: Read + Seek>(r: &mut R, file_size: u64) -> Result<WavLayout, String> {
    if file_size < 12 {
        return Err(format!("file too small for a RIFF header ({file_size} bytes)"));
    }

    let mut riff = [0u8; 12];
    r.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    r.read_exact(&mut riff).map_err(|e| e.to_string())?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".to_string());
    }
    let declared_riff_size = u32::from_le_bytes([riff[4], riff[5], riff[6], riff[7]]);

    let mut frame_bytes: Option<u32> = None;
    let mut pos: u64 = 12;

    loop {
        if pos + 8 > file_size {
            // Ran off the end before reaching `data`: the header itself was
            // truncated by the crash. Nothing we can safely reconstruct.
            return Err("no `data` chunk found (header truncated)".to_string());
        }
        let mut hdr = [0u8; 8];
        r.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
        r.read_exact(&mut hdr).map_err(|e| e.to_string())?;
        let ck_id = [hdr[0], hdr[1], hdr[2], hdr[3]];
        let ck_size = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);

        if &ck_id == b"data" {
            return Ok(WavLayout {
                data_size_off: pos + 4,
                data_off: pos + 8,
                declared_data_size: ck_size,
                declared_riff_size,
                frame_bytes: frame_bytes
                    .ok_or_else(|| "`data` chunk preceded by no `fmt ` chunk".to_string())?,
            });
        }

        if &ck_id == b"fmt " {
            // WAVEFORMAT/PCMWAVEFORMAT/WAVEFORMATEXTENSIBLE all share the same
            // first 16 bytes; nBlockAlign sits at fmt-data offset 12..14.
            if ck_size < 14 {
                return Err(format!("`fmt ` chunk too short ({ck_size} bytes)"));
            }
            let mut fmt = [0u8; 16];
            let n = ck_size.min(16) as usize;
            r.read_exact(&mut fmt[..n]).map_err(|e| e.to_string())?;
            let block_align = u16::from_le_bytes([fmt[12], fmt[13]]) as u32;
            if block_align == 0 {
                return Err("`fmt ` nBlockAlign is 0".to_string());
            }
            frame_bytes = Some(block_align);
        }

        // Advance past this chunk (chunks are word-aligned: pad byte if odd).
        let advance = 8 + u64::from(ck_size) + u64::from(ck_size & 1);
        pos = pos
            .checked_add(advance)
            .ok_or_else(|| "chunk size overflow".to_string())?;
    }
}

/// Make `path` a valid, playable WAV. Idempotent: a finalized file (or one
/// already reconstructed) is detected and left byte-for-byte untouched.
///
/// Only `hound`-produced `recording.wav` files are expected here, where `data`
/// is the final chunk, so the true payload length is `file_size - data_off`
/// floored to a whole frame. A trailing partial frame (the `BufWriter` buffer
/// lost on `SIGKILL`) is truncated so `hound::WavReader` accepts the result.
pub fn reconstruct_wav(path: &Path) -> Result<WavRecovery, String> {
    let mut f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    let file_size = f.metadata().map_err(|e| e.to_string())?.len();

    let layout = parse_wav_layout(&mut f, file_size)?;

    let frame = u64::from(layout.frame_bytes);
    let avail = file_size.saturating_sub(layout.data_off);
    let true_payload = avail / frame * frame; // floor to a whole frame
    let frames = true_payload / frame;

    let want_data = u32::try_from(true_payload).unwrap_or(u32::MAX);
    let want_riff =
        u32::try_from((layout.data_off + true_payload).saturating_sub(8)).unwrap_or(u32::MAX);

    let already = u64::from(layout.declared_data_size) == true_payload
        && layout.declared_riff_size == want_riff
        && file_size == layout.data_off + true_payload;
    if already {
        return Ok(WavRecovery::AlreadyValid { frames });
    }

    // Drop any trailing partial-frame bytes so the lengths we write are exact.
    if file_size != layout.data_off + true_payload {
        f.set_len(layout.data_off + true_payload)
            .map_err(|e| format!("truncate {}: {e}", path.display()))?;
    }
    f.seek(SeekFrom::Start(layout.data_size_off))
        .map_err(|e| e.to_string())?;
    f.write_all(&want_data.to_le_bytes())
        .map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(4)).map_err(|e| e.to_string())?;
    f.write_all(&want_riff.to_le_bytes())
        .map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;

    Ok(WavRecovery::Reconstructed { frames })
}

/// Tally of what the startup pass did (for logging / tests).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryReport {
    pub scanned: usize,
    pub wav_reconstructed: usize,
    pub wav_already_valid: usize,
    pub wav_failed: usize,
    pub rows_recreated: usize,
}

/// Startup recovery pass. Scans `meetings_dir` for `*/recording.wav`, makes
/// each WAV valid, and ensures a `meetings` row exists for it. Best-effort:
/// per-entry failures are logged and never abort core startup. Idempotent.
pub async fn run_recovery(meetings_dir: &Path, repo: &dyn MeetingRepo) -> RecoveryReport {
    let mut report = RecoveryReport::default();

    let entries = match std::fs::read_dir(meetings_dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("recovery: cannot read meetings dir {}: {e}", meetings_dir.display());
            return report;
        }
    };

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let audio_path = dir.join("recording.wav");
        if !audio_path.is_file() {
            continue;
        }
        report.scanned += 1;

        match reconstruct_wav(&audio_path) {
            Ok(WavRecovery::AlreadyValid { .. }) => report.wav_already_valid += 1,
            Ok(WavRecovery::Reconstructed { frames }) => {
                report.wav_reconstructed += 1;
                tracing::info!(
                    "recovery: reconstructed {} ({frames} frames)",
                    audio_path.display()
                );
            }
            Err(e) => {
                report.wav_failed += 1;
                tracing::warn!("recovery: cannot reconstruct {}: {e}", audio_path.display());
            }
        }

        // DB reconciliation — recreate a row only if none references this WAV.
        match repo.find_by_audio_path(&audio_path).await {
            Ok(Some(_)) => {} // row already exists → idempotent no-op
            Ok(None) => {
                let meeting = meeting_from_slug(&dir, &audio_path);
                match repo.save(&meeting).await {
                    Ok(()) => {
                        report.rows_recreated += 1;
                        tracing::info!(
                            "recovery: recreated meetings row for {}",
                            audio_path.display()
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "recovery: cannot recreate row for {}: {e}",
                            audio_path.display()
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "recovery: lookup failed for {}: {e}",
                    audio_path.display()
                );
            }
        }
    }

    if report.scanned > 0 {
        tracing::info!(
            "recovery: scanned={} reconstructed={} already_valid={} failed={} rows_recreated={}",
            report.scanned,
            report.wav_reconstructed,
            report.wav_already_valid,
            report.wav_failed,
            report.rows_recreated,
        );
    }
    report
}

/// Rebuild a `Meeting` from the directory slug
/// `<YYYY-MM-DD_HH-MM_uuid8>` (the format `start_recording::slug_from_meeting`
/// produces). The original full UUID is unrecoverable from the 8-char prefix,
/// so a fresh id is minted; `audio_path` is the dedupe key, not the id. If the
/// slug is unparseable the recording must still surface, so we fall back to the
/// file mtime / now and the raw directory name.
fn meeting_from_slug(dir: &Path, audio_path: &Path) -> Meeting {
    let slug = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("recovered");

    let created_at = parse_slug_timestamp(slug).unwrap_or_else(|| {
        std::fs::metadata(audio_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or_else(meeting_core::entities::now_unix)
    });

    Meeting {
        id: uuid::Uuid::new_v4().to_string(),
        name: human_name(created_at),
        meeting_dir: dir.to_path_buf(),
        audio_path: audio_path.to_path_buf(),
        transcript_path: None,
        protocol_path: None,
        transcript_text: None,
        protocol_text: None,
        created_at,
    }
}

/// Parse the UTC timestamp out of a `<YYYY-MM-DD_HH-MM_uuid8>` slug. Inverse of
/// `start_recording::slug_from_meeting` (which derives Y/M/D/H/M from a UTC
/// epoch, truncating seconds — so this round-trips to minute precision).
fn parse_slug_timestamp(slug: &str) -> Option<i64> {
    let mut it = slug.split('_');
    let date = it.next()?; // YYYY-MM-DD
    let hm = it.next()?; // HH-MM

    let mut d = date.split('-');
    let y: i64 = d.next()?.parse().ok()?;
    let mo: i64 = d.next()?.parse().ok()?;
    let da: i64 = d.next()?.parse().ok()?;

    let mut t = hm.split('-');
    let h: i64 = t.next()?.parse().ok()?;
    let mi: i64 = t.next()?.parse().ok()?;

    if !(1..=12).contains(&mo) || !(1..=31).contains(&da) || h > 23 || mi > 59 {
        return None;
    }
    Some(days_from_civil(y, mo, da) * 86_400 + h * 3_600 + mi * 60)
}

/// Days since the Unix epoch for a civil (proleptic Gregorian) date.
/// Howard Hinnant's `days_from_civil` — the exact inverse of the
/// `epoch_to_ymd` used by `start_recording::slug_from_meeting`.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Human-readable `YYYY-MM-DD HH:MM` (UTC) name, matching the
/// `start_recording::chrono_name` convention for fresh recordings.
fn human_name(ts: i64) -> String {
    let mins = ts / 60 % 60;
    let hours = ts / 3600 % 24;
    let days = ts / 86400;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {hours:02}:{mins:02}")
}

/// Inverse of `days_from_civil` (mirrors `start_recording::epoch_to_ymd`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::db::{Db, SqliteMeetingRepo};

    // ── Helpers: produce a *real codebase-produced* hound f32 WAV ────────────
    //
    // `cpal_capture::record_single` is literally:
    //   hound::WavWriter::new(BufWriter::new(file), spec) → write_sample* →
    //   finalize(). We use the same hound API + the same Float/32-bit spec, so
    //   these fixtures are byte-identical to what the recorder writes — not a
    //   hand-rolled 44-byte assumption.

    fn spec(channels: u16) -> hound::WavSpec {
        hound::WavSpec {
            channels,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        }
    }

    /// Finalized (clean-stop) WAV with `frames` stereo sample frames.
    fn write_finalized(path: &Path, channels: u16, frames: u32) {
        let mut w = hound::WavWriter::create(path, spec(channels)).unwrap();
        for i in 0..frames {
            for c in 0..channels {
                w.write_sample((i as f32 + c as f32) * 1e-3).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    /// Independent oracle: byte-scan for the `data` chunk id from offset 12
    /// (numeric fmt fields never spell ASCII "data"). Deliberately *not*
    /// `parse_wav_layout`, so the parser is verified against an external check.
    fn find_data_chunk_id_offset(bytes: &[u8]) -> usize {
        (12..bytes.len() - 4)
            .find(|&i| &bytes[i..i + 4] == b"data")
            .expect("hound WAV must contain a `data` chunk")
    }

    /// Simulate a mid-recording `SIGKILL`. hound's `write_headers` writes the
    /// RIFF + `data` `ckSize` fields as placeholder **0** and only
    /// `update_header()` (called by `flush`/`finalize`) patches them. A crash
    /// before any clean stop therefore leaves the sample bytes on disk with
    /// both length fields still 0. We reproduce that exact on-disk state by
    /// taking a real codebase-produced hound file (same hound API + Float/32
    /// spec as `record_single`) and stamping those two fields back to 0 —
    /// byte-identical to the genuine crash artifact.
    fn write_unfinalized(path: &Path, channels: u16, frames: u32) {
        write_finalized(path, channels, frames);
        let mut bytes = std::fs::read(path).unwrap();
        let data_id = find_data_chunk_id_offset(&bytes);
        bytes[4..8].fill(0); // top-level RIFF ckSize → placeholder 0
        bytes[data_id + 4..data_id + 8].fill(0); // `data` ckSize → placeholder 0
        std::fs::write(path, &bytes).unwrap();
    }

    fn read_back(path: &Path) -> (u32, u32, usize) {
        let r = hound::WavReader::open(path).unwrap();
        let frames = r.duration();
        let rate = r.spec().sample_rate;
        let n = r.into_samples::<f32>().filter_map(Result::ok).count();
        (frames, rate, n)
    }

    // ── THE critical acceptance test: record → truncate → reconstruct → assert
    #[test]
    fn record_truncate_reconstruct_yields_playable_wav_with_expected_frames() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.wav");

        // "record" 1000 stereo frames via the real hound API, no finalize.
        write_unfinalized(&path, 2, 1000);

        // The crash artifact has data ckSize 0: hound "reads" it but as an
        // empty 0-frame file — the recording is effectively lost pre-recovery.
        assert_eq!(
            read_back(&path),
            (0, 48_000, 0),
            "unfinalized WAV must expose no audio before recovery"
        );

        // Now also "truncate" mid-frame (lose 3 bytes of the last 8-byte
        // stereo frame) to exercise the partial-frame floor.
        {
            let f = OpenOptions::new().write(true).open(&path).unwrap();
            let len = f.metadata().unwrap().len();
            f.set_len(len - 3).unwrap();
        }

        let res = reconstruct_wav(&path).unwrap();
        assert_eq!(res, WavRecovery::Reconstructed { frames: 999 });

        // Playable, with the floored frame count and all samples intact.
        let (frames, rate, n) = read_back(&path);
        assert_eq!(frames, 999, "frame count must floor the partial frame");
        assert_eq!(rate, 48_000);
        assert_eq!(n, 999 * 2, "all whole-frame samples must be readable");
    }

    #[test]
    fn reconstruct_is_idempotent_and_byte_stable_on_second_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.wav");
        write_unfinalized(&path, 2, 512);

        let first = reconstruct_wav(&path).unwrap();
        assert!(matches!(first, WavRecovery::Reconstructed { frames: 512 }));
        let bytes_after_first = std::fs::read(&path).unwrap();

        // Second run: nothing to do, and not a single byte changes.
        let second = reconstruct_wav(&path).unwrap();
        assert_eq!(second, WavRecovery::AlreadyValid { frames: 512 });
        assert_eq!(std::fs::read(&path).unwrap(), bytes_after_first);
    }

    #[test]
    fn already_finalized_wav_is_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.wav");
        write_finalized(&path, 1, 777);
        let before = std::fs::read(&path).unwrap();

        let res = reconstruct_wav(&path).unwrap();
        assert_eq!(res, WavRecovery::AlreadyValid { frames: 777 });
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn parser_handles_multichannel_waveformatextensible_header_not_44_bytes() {
        // >2 channels → hound emits a 40-byte `fmt ` (WAVEFORMATEXTENSIBLE),
        // a 68-byte header. A 44-byte assumption would find garbage; the
        // chunk-walking parser must still locate `data`.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.wav");
        write_unfinalized(&path, 4, 300);

        let res = reconstruct_wav(&path).unwrap();
        assert_eq!(res, WavRecovery::Reconstructed { frames: 300 });
        let (frames, _, n) = read_back(&path);
        assert_eq!(frames, 300);
        assert_eq!(n, 300 * 4);
    }

    // ── Slug ↔ timestamp round-trip (must invert start_recording's logic) ────

    #[test]
    fn slug_timestamp_round_trips_start_recordings_format() {
        // 2026-05-10 14:30:00 UTC — same case asserted in start_recording.
        let ts = 1_778_423_400i64;
        let slug = "2026-05-10_14-30_a1b2c3d4";
        assert_eq!(parse_slug_timestamp(slug), Some(ts));
        assert_eq!(human_name(ts), "2026-05-10 14:30");
    }

    #[test]
    fn slug_timestamp_rejects_garbage() {
        assert_eq!(parse_slug_timestamp("not-a-slug"), None);
        assert_eq!(parse_slug_timestamp("2026-13-99_99-99_x"), None);
    }

    // ── run_recovery: both orphan kinds + double-run no-op ───────────────────

    fn slug_dir(root: &Path, slug: &str) -> PathBuf {
        let d = root.join(slug);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[tokio::test]
    async fn run_recovery_handles_both_orphan_kinds_and_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let repo = SqliteMeetingRepo(Db::open_in_memory().unwrap());

        // Orphan kind (a): no DB row at all.
        let dir_a = slug_dir(root.path(), "2026-05-10_14-30_aaaaaaaa");
        write_unfinalized(&dir_a.join("recording.wav"), 2, 400);

        // Orphan kind (b): unfinalized header BUT a meetings row exists.
        let dir_b = slug_dir(root.path(), "2026-05-11_09-15_bbbbbbbb");
        let wav_b = dir_b.join("recording.wav");
        write_unfinalized(&wav_b, 1, 250);
        let existing = Meeting {
            id: "row-b".to_string(),
            name: "kept".to_string(),
            meeting_dir: dir_b.clone(),
            audio_path: wav_b.clone(),
            transcript_path: None,
            protocol_path: None,
            transcript_text: None,
            protocol_text: None,
            created_at: 1,
        };
        repo.save(&existing).await.unwrap();

        let r1 = run_recovery(root.path(), &repo).await;
        assert_eq!(r1.scanned, 2);
        assert_eq!(r1.wav_reconstructed, 2, "both WAVs finalized");
        assert_eq!(r1.rows_recreated, 1, "only kind (a) needs a new row");

        // Both WAVs now playable with the right frame counts.
        assert_eq!(read_back(&dir_a.join("recording.wav")).0, 400);
        assert_eq!(read_back(&wav_b).0, 250);

        // Kind (a) is now listable; kind (b) row preserved (not duplicated).
        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        let kept = all.iter().find(|m| m.id == "row-b").unwrap();
        assert_eq!(kept.name, "kept", "existing row must be left intact");

        // Double-run: pure no-op — no reconstruction, no new rows.
        let r2 = run_recovery(root.path(), &repo).await;
        assert_eq!(r2.scanned, 2);
        assert_eq!(r2.wav_reconstructed, 0);
        assert_eq!(r2.wav_already_valid, 2);
        assert_eq!(r2.rows_recreated, 0);
        assert_eq!(repo.list_all().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn run_recovery_recreated_row_uses_slug_timestamp() {
        let root = tempfile::tempdir().unwrap();
        let repo = SqliteMeetingRepo(Db::open_in_memory().unwrap());

        let dir = slug_dir(root.path(), "2026-05-10_14-30_a1b2c3d4");
        write_unfinalized(&dir.join("recording.wav"), 2, 100);

        run_recovery(root.path(), &repo).await;

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].created_at, 1_778_423_400, "created_at from slug");
        assert_eq!(all[0].name, "2026-05-10 14:30");
    }

    #[tokio::test]
    async fn run_recovery_tolerates_missing_meetings_dir() {
        let repo = SqliteMeetingRepo(Db::open_in_memory().unwrap());
        let report = run_recovery(Path::new("/no/such/dir"), &repo).await;
        assert_eq!(report, RecoveryReport::default());
    }
}
