#!/usr/bin/env bash
# Build the Rust FFI shared library (libmeeting_assistant_ffi.so).
#
# Usage: ./build-ffi.sh [--debug]
#   --debug   Build in debug mode (faster compile, slower runtime)
#
# The .so ends up in rust/target/release/ or rust/target/debug/.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$SCRIPT_DIR/rust"

PROFILE="release"
CARGO_FLAGS=(--release)

for arg in "$@"; do
    case "$arg" in
        --debug) PROFILE="debug"; CARGO_FLAGS=() ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

case "$(uname -s)" in
    Darwin) LIB_EXT="dylib" ;;
    *)      LIB_EXT="so" ;;
esac

echo "→ Building Rust workspace ($PROFILE)..."
cargo build "${CARGO_FLAGS[@]}" --manifest-path "$RUST_DIR/Cargo.toml"

SO="$RUST_DIR/target/$PROFILE/libmeeting_assistant_ffi.$LIB_EXT"
echo "✓ $SO"
