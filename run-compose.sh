#!/usr/bin/env bash
# Build Rust FFI (if needed) and run the Compose Desktop UI.
#
# Usage: ./run-compose.sh [--debug] [--skip-build]
#   --debug       Use debug build of the Rust library (default: release)
#   --skip-build  Skip cargo build (use existing .so)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$SCRIPT_DIR/rust"
UI_DIR="$SCRIPT_DIR/ui-compose"

PROFILE="release"
SKIP_BUILD=0

for arg in "$@"; do
    case "$arg" in
        --debug)      PROFILE="debug" ;;
        --skip-build) SKIP_BUILD=1 ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

BINDINGS_KT="$UI_DIR/shared/src/desktopMain/kotlin/uniffi/meeting_assistant_ffi/meeting_assistant_ffi.kt"

if [[ $SKIP_BUILD -eq 0 ]]; then
    RELEASE_FLAG="--release"
    [[ "$PROFILE" == "debug" ]] && RELEASE_FLAG=""
    echo "→ Building Rust workspace ($PROFILE)..."
    cargo build $RELEASE_FLAG --manifest-path "$RUST_DIR/Cargo.toml"
    echo "✓ Rust built"

    echo "→ Regenerating Kotlin bindings..."
    TMPDIR="/tmp/uniffi-bindings-$$"
    (cd "$RUST_DIR" && cargo run --bin uniffi-bindgen -- \
        generate --library "target/$PROFILE/libmeeting_assistant_ffi.so" \
        --language kotlin --out-dir "$TMPDIR" 2>&1 | grep -v "ktlint")
    cp "$TMPDIR/uniffi/meeting_assistant_ffi/meeting_assistant_ffi.kt" "$BINDINGS_KT"
    rm -rf "$TMPDIR"
    echo "✓ Bindings updated"
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
