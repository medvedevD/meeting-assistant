# Whisper GPU Acceleration

## Context

`whisper-rs` is currently built CPU-only. Transcription dominates end-to-end
latency on long meetings, and modern machines have accelerators sitting idle:
Metal on macOS, CUDA on NVIDIA, Vulkan on cross-platform. `whisper.cpp`
(underneath `whisper-rs`) supports all three behind cargo features.

## Goal

Make accelerated transcription available where the hardware allows, without
breaking the CPU build path that the current packaging pipeline assumes.

## Sketch

- Add a `whisper-accelerate` cargo feature in
  [`meeting-adapters`](../rust/crates/adapters/Cargo.toml) that wires Metal on
  macOS by default (cheapest win — no extra runtime install).
- Document CUDA path for Linux/Windows users behind an opt-in feature; do not
  enable in the default packaging build to keep notarization/signing simple.
- Surface the active backend in the transcription-models settings screen so
  users see whether they got the accelerated path.
- Bench against a reference 60-minute WAV; record the speedup in the plan
  artifact for posterity.

## Expected Outcome

macOS users see materially faster transcription on M-series hardware out of
the box; CUDA users can opt in without rebuilding the world; CPU fallback
remains the default for builds that have to be portable.
