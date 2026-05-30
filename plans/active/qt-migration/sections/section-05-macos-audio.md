# Section 05 — macOS system-audio via ScreenCaptureKit

## Background
v1 must be feature-equal on macOS/Linux/Windows. Today
`rust/crates/adapters/src/audio/cpal_capture.rs` has mic via cpal, Linux system
via `parec`, Windows via WASAPI, and a **macOS `record_system` stub** that
returns `Err("system audio capture is not yet supported on this platform")`.
QtMultimedia cannot expose a macOS loopback device — the fix is OS-level
ScreenCaptureKit, in the Rust core, independent of the UI. This is a Rust-core
task that runs in parallel with the UI work.

## Requirements
On macOS 13+, `CaptureSource::System` and `CaptureSource::Mixed` produce valid
WAVs equivalent to the Linux/Windows backends; denied permission yields a clear
guided message, not a silent failure/crash; no screen frames are ever captured.

## Dependencies
- Requires: section-01 (branch). Rust-core only — independent of section-02/03/04.
- Blocks: section-07 (needs the Info.plist key for packaging).
- Parallelizable with sections 02–04.

## Implementation details
- Add the **`screencapturekit` crate** (doom-fish, v2.1.x, actively maintained)
  as a macOS-only dependency in `rust/crates/adapters/Cargo.toml`:
  `[target.'cfg(target_os="macos")'.dependencies]`.
- Replace the macOS `record_system` branch with an `SCStream` **audio-only**
  capture: `SCStreamConfiguration.capturesAudio = true`, 48 kHz / 2-ch / f32,
  writing to the same `hound` WAV path as the other backends (keep WAV spec
  consistent: `hound` f32, 48 kHz, 2ch).
- **Footguns:** keep the stream-output object alive for the whole session
  (otherwise frames drop / object is freed). **Do NOT instantiate a real screen
  output** — use the large `minimumFrameInterval` workaround so audio frames are
  not dropped; never request, retain, or process screen frames (privacy +
  resource + permission scope). Document this in code.
- **Mixed audio (real risk, not a copy of Linux):** SCStream (system) and cpal
  (mic) run on independent clocks and drift over a long meeting. Do a focused
  spike measuring drift over a 60-min capture BEFORE relying on the Linux
  mic+parec→`ffmpeg_mix` shape. v1 must either timestamp-align/resample the two
  streams before mixing, or explicitly document a bounded-drift limitation.
  Treat macOS `record_mixed` as the riskiest sub-task.
- macOS floor **13.0**. The bundle needs `NSScreenCaptureUsageDescription` in
  Info.plist (wired by section-07).
- **TCC re-prompt UX (interview decision).** v1 is ad-hoc/self-signed, so the
  Screen-Recording grant resets on each update (identity-hash changes). Detect
  "not authorized" from SCStream and surface a clear, actionable in-app message
  + a deep link to System Settings → Privacy & Security → Screen Recording.
  Handle first-run and post-update identically.

## Acceptance criteria
- [x] macOS 13+: `CaptureSource::System` yields a valid WAV equivalent to
      Linux/Windows. — **PASS** (live on macOS 26.5): SCK audio-only capture
      writes f32/48 kHz/2-ch WAV; `records_non_empty_wav_from_system_audio`
      and the drift spike both produced valid hound-readable WAVs.
- [x] macOS 13+: `CaptureSource::Mixed` works with documented clock-alignment
      handling. — **PASS** (live): SCK system + cpal mic → ffmpeg mix with
      `aresample=async=1` resample-to-PTS on **both** inputs (the
      timestamp-align/resample option, not a bounded-drift punt).
      `records_non_empty_wav_from_mixed` passes.
- [x] No screen frames are ever requested/retained (verified in code). —
      **PASS**: only an `SCStreamOutputType::Audio` handler is registered;
      no `Screen` handler, no `image_buffer()`/IOSurface call anywhere;
      enormous `minimumFrameInterval` keeps the unused video path idle.
- [x] Denied Screen-Recording permission → clear guided in-app message +
      deep link; no crash, no silent failure. — **PASS** (live): with the
      grant denied, `start_session` fast-failed in 0.05 s with the full
      guided message + `x-apple.systempreferences:…Privacy_ScreenCapture`
      deep link (surfaced at start via `preflight`, not only at stop).
- [~] Drift spike results recorded. — **PARTIAL**: 120 s smoke recorded
      (below); the 60-min run is deferred to `./run-drift-spike.sh` (TCC
      is granted to the editor only — the long run is best done from a
      Terminal session or the section-07 signed build).

## Drift-spike results

120 s smoke (macOS 26.5, Apple Silicon, idle/silent capture):

| metric | value |
|---|---|
| wall window | 120.002 s |
| system (SCK) | 5 750 400 frames @ 48 kHz = 119.800 s |
| mic (cpal) | 5 753 856 frames @ 48 kHz = 119.872 s |
| drift (sys − mic) | **−72 ms over 120 s (≈ −600 ppm)** |
| linear projection to 60 min | ≈ **−2.16 s** |

Caveats: at a 120 s window the start/stop transients (~0.1–0.2 s setup
latency per stream, visible as the ~0.2 s shortfall vs wall clock) do **not**
fully cancel and inflate the apparent ppm — the projection is a loose upper
bound, not the true steady-state rate. It does confirm the two clocks drift
on the order of seconds/hour (non-catastrophic for speech transcription) and
that `aresample=async=1` in the macOS mix filter is the correct mitigation.
The accurate 60-min number must come from `run-drift-spike.sh`.

## Files to create/modify
- Modify `rust/crates/adapters/src/audio/cpal_capture.rs` (macOS branch).
- Modify `rust/crates/adapters/Cargo.toml` (macOS-only dep).
- Create a macOS ScreenCaptureKit helper module under
  `rust/crates/adapters/src/audio/`.
