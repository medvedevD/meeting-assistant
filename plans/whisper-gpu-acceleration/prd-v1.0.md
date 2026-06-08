# Whisper GPU Acceleration — PRD v1.0

## Problem

`whisper-rs` was built CPU-only. Transcription dominates end-to-end latency on
long meetings while modern machines have accelerators sitting idle: Metal on
macOS, CUDA on NVIDIA, Vulkan cross-platform. `whisper.cpp` (underneath
`whisper-rs`) supports all three behind cargo features, but none were wired.

## Goal

Make accelerated transcription available where the hardware allows, **without
breaking the CPU build path** the packaging pipeline assumes, and surface the
active backend so users can see whether they got the accelerated path.

## Scope

**In**

- macOS builds default to the **Metal** backend (system framework — no extra
  runtime install, notarization-safe).
- Opt-in **CUDA** / **Vulkan** cargo features for portable (Linux/Windows)
  builds, **off by default**.
- A build-time–derived backend constant (`cpu`/`metal`/`cuda`/`vulkan`) exposed
  through the settings snapshot.
- A read-only backend badge in the transcription settings screen.
- A runtime **"use GPU acceleration"** toggle (settings) that forces CPU on an
  accelerated build; disabled/no-op on a CPU-only build.
- **Honest plain-language messaging**: on a CPU-only build the toggle is disabled
  with "GPU acceleration is not supported in this version yet" — no dev jargon, no
  promise of a build that does not ship (ADR-010).
- Benchmark methodology + result table in `plan-v1.0.md`.

**Out**

- Auto-selecting/auto-downloading a GPU runtime (driver/SDK) at install time.
- Runtime device probing beyond the compiled-backend report (whisper.cpp's own
  CPU fallback covers the "GPU runtime missing" case).
- Enabling CUDA/Vulkan in the default packaging build (keeps signing/notarization
  and CI portable).
- Per-model or per-job backend selection.

## Deliverables

| File | Change |
|---|---|
| `rust/crates/adapters/Cargo.toml` | `whisper-rs` moved to per-target tables (Metal default on macOS, plain elsewhere); `whisper-cuda`/`whisper-vulkan` opt-in features. |
| `rust/crates/adapters/build.rs` (new) | Derives `MEETING_WHISPER_BACKEND` from target OS + active features. |
| `rust/crates/adapters/src/whisper.rs` | `whisper_backend()` accessor; backend in bench log; `TranscriberPrefs.use_gpu` (threaded into the runner factory + context params, reload-on-change); regression tests. |
| `rust/crates/adapters/src/lib.rs` | Export `whisper_backend`. |
| `rust/crates/adapters/src/settings_store.rs` | `PersistedTranscriberPrefs.use_gpu` (default true) across struct/wire/Default. |
| `rust/crates/app/src/settings_service.rs` | Read-only `transcriber.backend` + `use_gpu` in the snapshot; `use_gpu` applied on hot-swap. |
| `rust/crates/app/src/container.rs` | Pass `use_gpu` into the initial `TranscriberPrefs`. |
| `qt-app/qml/screens/settings/WhisperPanel.qml` | Backend badge + "GPU-ускорение" switch with honest CPU-build help (`gpuHelp()`). |
| `rust/crates/app/Cargo.toml`, `run-qt.sh` | Passthrough `whisper-cuda`/`whisper-vulkan` features + `--cuda`/`--vulkan` build flags. |

**Test plan**

- Unit: `backend_constant_is_known` — value is one of the four; portable default
  build (non-macOS, no GPU feature) must report `cpu`.
- Contract: existing `settings_get_put_roundtrip_and_secret_flag` must stay
  green, proving the read-only `backend` field is ignored on PUT.
- Manifest: `cargo metadata` confirms `whisper-cuda`/`whisper-vulkan` resolve to
  the right `whisper-rs` features.

## Closing step

Make the closing commit that performs
`git mv plans/active/whisper-gpu-acceleration plans/done/whisper-gpu-acceleration`
after the implementation is committed, tests pass on the working branch, and the
macOS Metal speedup row in `plan-v1.0.md` has been filled in on real M-series
hardware.
