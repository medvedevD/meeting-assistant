#!/usr/bin/env bash
# Build the Rust FFI shared library (libmeeting_assistant_ffi.so).
#
# Usage: ./build-ffi.sh [--release]
#   --release   Build in release mode (optimized, slower compile)
#
# The .so ends up in rust/target/debug/ or rust/target/release/.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$SCRIPT_DIR/rust"

PROFILE="debug"
CARGO_FLAGS=()

for arg in "$@"; do
    case "$arg" in
        --release) PROFILE="release"; CARGO_FLAGS+=(--release) ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

echo "→ Building Rust workspace ($PROFILE)..."
cargo build "${CARGO_FLAGS[@]}" --manifest-path "$RUST_DIR/Cargo.toml"

SO="$RUST_DIR/target/$PROFILE/libmeeting_assistant_ffi.so"
echo "✓ $SO"
