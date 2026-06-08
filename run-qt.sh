#!/usr/bin/env bash
# Canonical dev workflow.
#
# Builds the Rust `meeting-server` sidecar + the Qt GUI, copies the sidecar
# next to the GUI binary (so SidecarManager finds it via applicationDirPath),
# then runs the GUI.
#
# Usage: ./run-qt.sh [--debug] [--skip-rust] [--skip-build] [--no-run] [--cuda|--vulkan]
#   --debug       Debug build of the Rust sidecar (default: release)
#   --skip-rust   Don't rebuild the Rust sidecar
#   --skip-build  Don't reconfigure/rebuild the Qt app
#   --no-run      Build only; do not launch the GUI
#   --cuda        Build the sidecar with the CUDA Whisper backend (needs CUDA Toolkit)
#   --vulkan      Build the sidecar with the Vulkan Whisper backend (needs Vulkan SDK)
#
# Dev-only:
#   FIRST_RUN=1 ./run-qt.sh
#       Launch with fresh isolated temporary XDG data/config dirs so the app
#       behaves like a clean first install without touching real meetings/settings.
#
# Qt discovery (first hit wins):
#   $CMAKE_PREFIX_PATH  →  $QT_DIR  →  ~/Qt/<ver>/<kit>  →  qmake6 on PATH
#                       →  brew --prefix qt
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$SCRIPT_DIR/rust"
QT_APP_DIR="$SCRIPT_DIR/qt-app"
BUILD_DIR="$QT_APP_DIR/build"

PROFILE="release"
SKIP_RUST=0
SKIP_BUILD=0
RUN=1
WHISPER_FEATURE=""

for arg in "$@"; do
    case "$arg" in
        --debug)      PROFILE="debug" ;;
        --skip-rust)  SKIP_RUST=1 ;;
        --skip-build) SKIP_BUILD=1 ;;
        --no-run)     RUN=0 ;;
        --cuda)       WHISPER_FEATURE="whisper-cuda" ;;
        --vulkan)     WHISPER_FEATURE="whisper-vulkan" ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

case "$(uname -s)" in
    Darwin) QT_KIT="macos" ;;
    Linux)  QT_KIT="gcc_64" ;;
    *)      QT_KIT="" ;;
esac

# ── Locate Qt 6 ──────────────────────────────────────────────────────────────
detect_qt_prefix() {
    if [[ -n "${CMAKE_PREFIX_PATH:-}" ]]; then echo "$CMAKE_PREFIX_PATH"; return; fi
    if [[ -n "${QT_DIR:-}" ]];           then echo "$QT_DIR";           return; fi
    if [[ -n "$QT_KIT" && -d "$HOME/Qt" ]]; then
        local v
        v="$(ls -1 "$HOME/Qt" 2>/dev/null | grep -E '^6\.' | sort -V | tail -1 || true)"
        if [[ -n "$v" && -d "$HOME/Qt/$v/$QT_KIT" ]]; then
            echo "$HOME/Qt/$v/$QT_KIT"; return
        fi
    fi
    if command -v qmake6 >/dev/null 2>&1; then
        dirname "$(dirname "$(qmake6 -query QT_INSTALL_PREFIX 2>/dev/null)/.")"; return
    fi
    if command -v brew >/dev/null 2>&1 && brew --prefix qt >/dev/null 2>&1; then
        brew --prefix qt; return
    fi
    echo ""
}

# ── Rust sidecar ─────────────────────────────────────────────────────────────
if [[ $SKIP_RUST -eq 0 ]]; then
    RELEASE_FLAG="--release"
    [[ "$PROFILE" == "debug" ]] && RELEASE_FLAG=""
    FEATURE_FLAG=""
    if [[ -n "$WHISPER_FEATURE" ]]; then
        FEATURE_FLAG="--features $WHISPER_FEATURE"
        echo "→ Building meeting-server sidecar ($PROFILE, $WHISPER_FEATURE)..."
    else
        echo "→ Building meeting-server sidecar ($PROFILE)..."
    fi
    cargo build $RELEASE_FLAG $FEATURE_FLAG --manifest-path "$RUST_DIR/Cargo.toml" --bin meeting-server
    echo "✓ Sidecar built"
fi

SIDECAR="$RUST_DIR/target/$PROFILE/meeting-server"
if [[ ! -x "$SIDECAR" ]]; then
    echo "Error: $SIDECAR not found/executable. Run without --skip-rust." >&2
    exit 1
fi

# ── Qt GUI ───────────────────────────────────────────────────────────────────
if [[ $SKIP_BUILD -eq 0 ]]; then
    QT_PREFIX="$(detect_qt_prefix)"
    if [[ -z "$QT_PREFIX" ]]; then
        echo "Error: could not locate Qt 6. Set CMAKE_PREFIX_PATH or QT_DIR." >&2
        exit 1
    fi
    echo "→ Qt prefix: $QT_PREFIX"

    CMAKE_GEN=()
    command -v ninja >/dev/null 2>&1 && CMAKE_GEN=(-G Ninja)

    echo "→ Configuring qt-app..."
    cmake -S "$QT_APP_DIR" -B "$BUILD_DIR" "${CMAKE_GEN[@]}" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_PREFIX_PATH="$QT_PREFIX"
    echo "→ Building qt-app..."
    cmake --build "$BUILD_DIR" --parallel
    echo "✓ Qt app built"
fi

# Resolve the GUI binary (plain executable; no .app bundle in dev).
GUI_BIN="$BUILD_DIR/meeting-assistant-qt"
[[ -x "$GUI_BIN" ]] || GUI_BIN="$(find "$BUILD_DIR" -maxdepth 2 -name 'meeting-assistant-qt' -perm -111 -type f 2>/dev/null | head -1 || true)"
if [[ -z "$GUI_BIN" || ! -x "$GUI_BIN" ]]; then
    echo "Error: GUI binary not found under $BUILD_DIR. Run without --skip-build." >&2
    exit 1
fi

# Co-locate the sidecar with the GUI — SidecarManager looks in
# applicationDirPath() (mirrors the section-07 packaging layout).
cp -f "$SIDECAR" "$(dirname "$GUI_BIN")/meeting-server"
echo "✓ Sidecar staged next to GUI: $(dirname "$GUI_BIN")/meeting-server"

if [[ $RUN -eq 0 ]]; then
    echo "→ Build complete (--no-run). GUI: $GUI_BIN"
    exit 0
fi

if [[ "${FIRST_RUN:-}" == "1" ]]; then
    if [[ -z "${FIRST_RUN_DIR:-}" ]]; then
        FIRST_RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/meeting-assistant-first-run.XXXXXX")"
    fi
    mkdir -p "$FIRST_RUN_DIR/data" "$FIRST_RUN_DIR/config"
    export XDG_DATA_HOME="$FIRST_RUN_DIR/data"
    export XDG_CONFIG_HOME="$FIRST_RUN_DIR/config"
    echo "→ FIRST_RUN=1: using isolated app data under $FIRST_RUN_DIR"
fi

echo "→ Launching Meeting Assistant (Qt)..."
exec "$GUI_BIN"
