#!/usr/bin/env bash
# Build Rust FFI (if needed) and run the Compose Desktop UI.
#
# Usage: ./run-compose.sh [--release] [--skip-build]
#   --release     Use release build of the Rust library
#   --skip-build  Skip cargo build (use existing .so)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$SCRIPT_DIR/rust"
UI_DIR="$SCRIPT_DIR/ui-compose"

PROFILE="debug"
SKIP_BUILD=0

for arg in "$@"; do
    case "$arg" in
        --release)    PROFILE="release" ;;
        --skip-build) SKIP_BUILD=1 ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

if [[ $SKIP_BUILD -eq 0 ]]; then
    RELEASE_FLAG=""
    [[ "$PROFILE" == "release" ]] && RELEASE_FLAG="--release"
    echo "→ Building Rust FFI ($PROFILE)..."
    cargo build -p ffi $RELEASE_FLAG --manifest-path "$RUST_DIR/Cargo.toml"
    echo "✓ Rust FFI built"
fi

SO="$RUST_DIR/target/$PROFILE/libmeeting_assistant_ffi.so"
if [[ ! -f "$SO" ]]; then
    echo "Error: $SO not found. Run without --skip-build." >&2
    exit 1
fi

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
    echo "Warning: ANTHROPIC_API_KEY is not set — protocol generation will fail."
fi

echo "→ Starting Compose Desktop UI..."
cd "$UI_DIR"
exec ./gradlew :desktopApp:run \
    -Drust.target.dir="$RUST_DIR/target/$PROFILE" \
    --quiet
