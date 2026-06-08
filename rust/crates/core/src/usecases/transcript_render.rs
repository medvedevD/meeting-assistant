//! Pure rendering of a [`Transcript`] into the human-readable `transcript.md`.
//!
//! The Whisper adapter already produces per-segment timestamps; this module is
//! the single place that turns them into the timestamped Markdown the user
//! opens. It does no I/O and no async work, so it is cheap to unit-test and
//! keeps formatting policy out of both the file-store adapter and the worker.
//!
//! The rendered Markdown is the *human* view. The *machine* view consumed by
//! the LLM (`generate_protocol`) stays as the flat `Transcript::text`, so
//! timestamps never leak into the protocol prompt.

use crate::entities::Transcript;

/// Gap between consecutive segments above which we insert a blank line, marking
/// a pause in the conversation. Small enough to break on real silences, large
/// enough not to fragment normal back-and-forth speech.
const PAUSE_GAP_MS: u64 = 3_000;

/// Metadata rendered into the transcript header. Borrowed — the caller owns the
/// meeting name and the already-formatted date string.
pub struct TranscriptMeta<'a> {
    pub title: &'a str,
    /// Pre-formatted date, e.g. `24.04.2026`.
    pub date: &'a str,
}

/// Render a transcript to Markdown: a header (title + date) followed by one
/// `[MM:SS] text` line per segment, with a blank line inserted wherever the
/// silence between two segments exceeds [`PAUSE_GAP_MS`].
///
/// Segments whose text is empty after trimming are skipped (Whisper can emit
/// blank segments over silence). When there are no usable segments, the body
/// falls back to the flat `transcript.text` so the file is never empty.
pub fn render_markdown(t: &Transcript, meta: &TranscriptMeta) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Транскрипция: {}\n", meta.title));
    out.push_str(&format!("**Дата:** {}\n\n", meta.date));

    let mut wrote_any = false;
    let mut prev_end_ms: Option<u64> = None;

    for seg in &t.segments {
        let text = seg.text.trim();
        if text.is_empty() {
            continue;
        }
        if let Some(prev_end) = prev_end_ms {
            if seg.start_ms.saturating_sub(prev_end) > PAUSE_GAP_MS {
                out.push('\n');
            }
        }
        out.push_str(&format!("[{}] {}\n", fmt_ts(seg.start_ms), text));
        prev_end_ms = Some(seg.end_ms);
        wrote_any = true;
    }

    // No usable segments (e.g. a fake transcriber, or an all-silence clip):
    // keep the flat text so the file still carries the content.
    if !wrote_any {
        let flat = t.text.trim();
        if !flat.is_empty() {
            out.push_str(flat);
            out.push('\n');
        }
    }

    out
}

/// Format a Unix timestamp (seconds) as `DD.MM.YYYY` for the transcript header.
/// Pure date math (no external date crate, matching the rest of the workspace).
pub fn format_date_dmy(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let (y, m, d) = epoch_to_ymd(days);
    format!("{d:02}.{m:02}.{y:04}")
}

/// Civil date from a day count since the Unix epoch. Algorithm from
/// <https://howardhinnant.github.io/date_algorithms.html> (same one used by
/// `start_recording::epoch_to_ymd`).
fn epoch_to_ymd(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Format a millisecond offset as `MM:SS`, rolling over to `HH:MM:SS` once the
/// offset reaches one hour. Minutes and seconds are always zero-padded.
fn fmt_ts(ms: u64) -> String {
    let total_secs = ms / 1_000;
    let secs = total_secs % 60;
    let mins = (total_secs / 60) % 60;
    let hours = total_secs / 3_600;
    if hours > 0 {
        format!("{hours:02}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Segment;

    fn seg(start_ms: u64, end_ms: u64, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms,
            text: text.to_string(),
        }
    }

    fn transcript(segments: Vec<Segment>, text: &str) -> Transcript {
        Transcript {
            text: text.to_string(),
            segments,
            language: "ru".into(),
        }
    }

    fn meta() -> TranscriptMeta<'static> {
        TranscriptMeta {
            title: "1x1-sergey-riazancev",
            date: "24.04.2026",
        }
    }

    #[test]
    fn header_is_rendered() {
        let t = transcript(vec![seg(0, 1000, "привет")], "привет");
        let md = render_markdown(&t, &meta());
        assert!(md.starts_with("# Транскрипция: 1x1-sergey-riazancev\n**Дата:** 24.04.2026\n\n"));
    }

    #[test]
    fn single_segment_has_timestamp() {
        let t = transcript(vec![seg(7_000, 11_000, "обсудить что мешает")], "");
        let md = render_markdown(&t, &meta());
        assert!(md.contains("[00:07] обсудить что мешает\n"), "got: {md}");
    }

    #[test]
    fn rolls_over_to_hours_past_one_hour() {
        let t = transcript(vec![seg(3_661_000, 3_662_000, "поздний сегмент")], "");
        let md = render_markdown(&t, &meta());
        // 3_661_000 ms = 1h 01m 01s
        assert!(md.contains("[01:01:01] поздний сегмент\n"), "got: {md}");
    }

    #[test]
    fn inserts_blank_line_on_long_pause() {
        let t = transcript(
            vec![
                seg(0, 7_000, "так начнём"),
                // gap 7_000 → 53_000 = 46s > 3s ⇒ blank line before
                seg(53_000, 57_000, "про текущие задачи"),
            ],
            "",
        );
        let md = render_markdown(&t, &meta());
        assert!(
            md.contains("[00:00] так начнём\n\n[00:53] про текущие задачи\n"),
            "got: {md}"
        );
    }

    #[test]
    fn no_blank_line_on_short_gap() {
        let t = transcript(
            vec![
                seg(0, 7_000, "так начнём"),
                // gap 7_000 → 8_000 = 1s ≤ 3s ⇒ no blank line
                seg(8_000, 11_000, "обсудить вопросы"),
            ],
            "",
        );
        let md = render_markdown(&t, &meta());
        assert!(
            md.contains("[00:00] так начнём\n[00:08] обсудить вопросы\n"),
            "got: {md}"
        );
    }

    #[test]
    fn trims_segment_text_and_skips_empty() {
        let t = transcript(
            vec![
                seg(0, 1_000, "  привет  "),
                seg(1_000, 2_000, "   "),
                seg(2_000, 3_000, "мир"),
            ],
            "привет мир",
        );
        let md = render_markdown(&t, &meta());
        assert!(md.contains("[00:00] привет\n"), "got: {md}");
        assert!(md.contains("[00:02] мир\n"), "got: {md}");
        assert!(!md.contains("[00:01]"), "empty segment must be skipped: {md}");
    }

    #[test]
    fn format_date_dmy_formats_known_epoch() {
        // 2026-04-24 00:00:00 UTC = 1_776_988_800
        assert_eq!(format_date_dmy(1_776_988_800), "24.04.2026");
        // Unix epoch day.
        assert_eq!(format_date_dmy(0), "01.01.1970");
    }

    #[test]
    fn falls_back_to_flat_text_without_segments() {
        let t = transcript(vec![], "сплошной текст без сегментов");
        let md = render_markdown(&t, &meta());
        assert!(md.contains("сплошной текст без сегментов\n"), "got: {md}");
    }
}
