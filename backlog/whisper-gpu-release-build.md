# GPU release build for Windows and Linux

## Problem

Release builds for Windows and Linux transcribe on the CPU, and the settings screen says
GPU acceleration is not supported yet. CUDA and Vulkan exist only as opt-in cargo features
for people building from source, so long meetings take longer than they need to.

## Sketch

Ship one Vulkan build as the accelerated artifact. It runs on NVIDIA, AMD, and Intel
through the driver, with no SDK on the user's machine. CUDA can stay an optional power
build, since Vulkan is slower for Whisper. This adds a packaging lane with the Vulkan SDK
and `glslc` on the build runner. Then update the note in `WhisperPanel.qml` and record the
speedup with `rust/scripts/bench_transcribe.sh`.
