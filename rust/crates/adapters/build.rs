//! Derives the compiled Whisper compute backend and exposes it to the crate as
//! the `MEETING_WHISPER_BACKEND` compile-time env var (read via `env!`).
//!
//! This is the single authoritative mirror of the backend-selection policy
//! encoded in `Cargo.toml`:
//!   * macOS links the Metal backend unconditionally (target dependency table);
//!   * `whisper-cuda` / `whisper-vulkan` features layer a GPU backend on top of
//!     the CPU build for portable (non-macOS) targets;
//!   * everything else is CPU-only.
//!
//! Keep this in sync with the `whisper-rs` dependency declarations in
//! `Cargo.toml`. See ADR-006 in `plans/active/whisper-gpu-acceleration`.

use std::env;

fn main() {
    // Explicit GPU opt-ins win over the implicit macOS Metal default so a build
    // that deliberately enables e.g. CUDA reports CUDA even on macOS.
    let backend = if env::var_os("CARGO_FEATURE_WHISPER_CUDA").is_some() {
        "cuda"
    } else if env::var_os("CARGO_FEATURE_WHISPER_VULKAN").is_some() {
        "vulkan"
    } else if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        "metal"
    } else {
        "cpu"
    };

    println!("cargo:rustc-env=MEETING_WHISPER_BACKEND={backend}");
    println!("cargo:rerun-if-changed=build.rs");
}
