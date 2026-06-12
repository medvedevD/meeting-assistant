#!/usr/bin/env bash
# Build the Linux two-binary artefact as a single self-contained AppImage:
#   cargo build (meeting-server)  →  cmake build + install into an AppDir
#   →  linuxdeploy + the Qt plugin (bundles Qt6 + the QML runtime)
#   →  MeetingAssistant-<arch>.AppImage
#
# $0 cost, no signing gate (locked decision). System audio already works on
# Linux via `parec` (PulseAudio) — no extra entitlement/permission plumbing.
#
# Usage: build-appimage.sh [--debug]
#
# Qt discovery: $CMAKE_PREFIX_PATH → $QT_DIR → ~/Qt/<ver>/gcc_64 → qmake6.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
RUST_DIR="$REPO/rust"
QT_APP_DIR="$REPO/qt-app"
BUILD_DIR="$QT_APP_DIR/build-linux-pkg"
DIST="$REPO/dist/linux"
APPDIR="$DIST/AppDir"
TOOLS="$DIST/.tools"
ARCH="$(uname -m)"

PROFILE="release"
for a in "$@"; do case "$a" in
    --debug) PROFILE="debug" ;;
    *) echo "Unknown arg: $a" >&2; exit 1 ;;
esac; done

detect_qt_prefix() {
    if [[ -n "${CMAKE_PREFIX_PATH:-}" ]]; then echo "$CMAKE_PREFIX_PATH"; return; fi
    if [[ -n "${QT_DIR:-}" ]];           then echo "$QT_DIR";           return; fi
    if [[ -d "$HOME/Qt" ]]; then
        local v
        v="$(ls -1 "$HOME/Qt" 2>/dev/null | grep -E '^6\.' | sort -V | tail -1 || true)"
        [[ -n "$v" && -d "$HOME/Qt/$v/gcc_64" ]] && { echo "$HOME/Qt/$v/gcc_64"; return; }
    fi
    command -v qmake6 >/dev/null 2>&1 && \
        { dirname "$(dirname "$(qmake6 -query QT_INSTALL_PREFIX)/.")"; return; }
    echo ""
}
QT_PREFIX="$(detect_qt_prefix)"
[[ -n "$QT_PREFIX" ]] || { echo "Error: Qt 6 not found (set CMAKE_PREFIX_PATH)." >&2; exit 1; }
echo "→ Qt prefix: $QT_PREFIX"

# ── 1. Rust sidecar (same revision as the GUI; version-pinned together) ──────
echo "→ cargo build meeting-server ($PROFILE)…"
REL_FLAG="--release"; [[ "$PROFILE" == debug ]] && REL_FLAG=""
cargo build $REL_FLAG --manifest-path "$RUST_DIR/Cargo.toml" --bin meeting-server
SIDECAR="$RUST_DIR/target/$PROFILE/meeting-server"
[[ -x "$SIDECAR" ]] || { echo "Error: sidecar not built: $SIDECAR" >&2; exit 1; }

# ── 2. Build + install into the AppDir (CMake Linux branch) ──────────────────
echo "→ cmake configure + build + install → AppDir/usr…"
rm -rf "$APPDIR"; mkdir -p "$APPDIR"
CMAKE_GEN=(); command -v ninja >/dev/null 2>&1 && CMAKE_GEN=(-G Ninja)
cmake -S "$QT_APP_DIR" -B "$BUILD_DIR" "${CMAKE_GEN[@]}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PREFIX_PATH="$QT_PREFIX" \
    -DMEETING_SERVER_BIN="$SIDECAR" \
    -DCMAKE_INSTALL_PREFIX="$APPDIR/usr"
cmake --build "$BUILD_DIR" --parallel
cmake --install "$BUILD_DIR"

# Icon where linuxdeploy + the FreeDesktop spec expect it.
ICON_DST="$APPDIR/usr/share/icons/hicolor/512x512/apps"
mkdir -p "$ICON_DST"
cp "$REPO/packaging/assets/meeting-assistant.png" "$ICON_DST/meeting-assistant.png"

[[ -x "$APPDIR/usr/bin/meeting-server" ]] || \
    { echo "Error: helper not installed beside GUI (CMake rule broken)" >&2; exit 1; }

# ── 3. Fetch linuxdeploy + the Qt plugin (cached) ────────────────────────────
mkdir -p "$TOOLS"
fetch() { # fetch <name> <url>
    local out="$TOOLS/$1"
    if [[ ! -x "$out" ]]; then
        echo "→ downloading $1…"
        curl -fL --retry 3 -o "$out" "$2"
        chmod +x "$out"
    fi
    echo "$out"
}
LD="$(fetch linuxdeploy-$ARCH.AppImage \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$ARCH.AppImage")"
LDQT="$(fetch linuxdeploy-plugin-qt-$ARCH.AppImage \
    "https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-$ARCH.AppImage")"

# ── 4. linuxdeploy + Qt plugin → AppImage ────────────────────────────────────
# QML_SOURCES_PATHS lets the Qt plugin scan our QML and bundle exactly the
# imports we use. qmake on PATH lets the plugin find the right Qt.
export QML_SOURCES_PATHS="$QT_APP_DIR/qml"
export PATH="$QT_PREFIX/bin:$PATH"
export OUTPUT="MeetingAssistant-$ARCH.AppImage"

# The in-card audio player (QtMultimedia) loads its backend from the
# `multimedia` plugin category, which linuxdeploy-plugin-qt does NOT bundle from
# a QML-import scan alone — the backend plugin lives under plugins/multimedia/,
# not in the QML tree. Force it in; its NEEDED ffmpeg libs (libav*) are then
# pulled by linuxdeploy's dependency resolver. Without this the player silently
# fails to play on a clean machine.
export EXTRA_QT_PLUGINS="multimedia"

echo "→ linuxdeploy (+ qt plugin)…"
mkdir -p "$DIST"
# No --custom-apprun: linuxdeploy + the qt plugin generate the AppRun and the
# apprun-hook that exports QT_PLUGIN_PATH / QML2_IMPORT_PATH / LD_LIBRARY_PATH.
# A custom AppRun would bypass that hook and break Qt/QML discovery. The GUI
# then finds the sidecar via its own exe dir (both in usr/bin).
( cd "$DIST" && "$LD" --appdir "$APPDIR" --plugin qt \
    --desktop-file "$APPDIR/usr/share/applications/meeting-assistant.desktop" \
    --icon-file "$ICON_DST/meeting-assistant.png" \
    --output appimage )

# Fail-fast: assert the multimedia backend + ffmpeg actually landed in the
# AppDir. A missing plugin here ships an app whose player is dead on every
# other machine — exactly the kind of plugin gap that bit the xcb deploy.
if ! find "$APPDIR" -name 'libffmpegmediaplugin.so' | grep -q .; then
    echo "Error: QtMultimedia ffmpeg plugin not bundled (EXTRA_QT_PLUGINS=multimedia did not take)." >&2
    exit 1
fi
if ! find "$APPDIR" -name 'libavcodec.so*' | grep -q .; then
    echo "Error: ffmpeg runtime libs (libavcodec) not bundled — player will fail to decode." >&2
    exit 1
fi
echo "→ QtMultimedia backend + ffmpeg libs bundled ✓"

echo "✓ AppImage: $DIST/$OUTPUT"
echo "  Both binaries (meeting-assistant-qt + meeting-server) live in"
echo "  usr/bin inside the AppImage; the GUI locates the sidecar via its own"
echo "  exe dir. Qt6 + QML runtime bundled by the qt plugin (dynamic link →"
echo "  LGPLv3-compliant; written offer in usr/share/doc/meeting-assistant)."
