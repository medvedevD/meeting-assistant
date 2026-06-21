#!/usr/bin/env bash
# Build the macOS two-binary artefact end-to-end:
#   cargo build (meeting-server)  →  cmake build MeetingAssistant.app
#   →  cmake --install (helper + licenses INTO the .app)
#   →  macdeployqt (Qt runtime; it does NOT copy the helper — install did)
#   →  ad-hoc deep-sign  →  verify  →  DMG (for the Homebrew Cask)
#
# This is also the EARLY macOS spike harness (section-07): it exercises the
# single most fragile chain — ad-hoc sign + nested-helper deep-sign + Qt
# deploy + Gatekeeper — before sections 03–05 go deep.
#
# Usage: build-app.sh [--debug] [--no-dmg]
#   --debug    debug Rust sidecar (default: release)
#   --no-dmg   stop after the signed .app (skip DMG)
#
# Qt discovery: $CMAKE_PREFIX_PATH → $QT_DIR → ~/Qt/<ver>/macos → qmake6/brew.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
RUST_DIR="$REPO/rust"
QT_APP_DIR="$REPO/qt-app"
BUILD_DIR="$QT_APP_DIR/build-macos-pkg"
DIST="$REPO/dist/macos"
STAGE="$DIST/stage"
TOOLS="$DIST/.tools"

PROFILE="release"
MAKE_DMG=1
for a in "$@"; do case "$a" in
    --debug)  PROFILE="debug" ;;
    --no-dmg) MAKE_DMG=0 ;;
    *) echo "Unknown arg: $a" >&2; exit 1 ;;
esac; done

# ── Locate Qt 6 (mirrors run-qt.sh) ──────────────────────────────────────────
detect_qt_prefix() {
    if [[ -n "${CMAKE_PREFIX_PATH:-}" ]]; then echo "$CMAKE_PREFIX_PATH"; return; fi
    if [[ -n "${QT_DIR:-}" ]];           then echo "$QT_DIR";           return; fi
    if [[ -d "$HOME/Qt" ]]; then
        local v
        v="$(ls -1 "$HOME/Qt" 2>/dev/null | grep -E '^6\.' | sort -V | tail -1 || true)"
        [[ -n "$v" && -d "$HOME/Qt/$v/macos" ]] && { echo "$HOME/Qt/$v/macos"; return; }
    fi
    command -v qmake6 >/dev/null 2>&1 && \
        { dirname "$(dirname "$(qmake6 -query QT_INSTALL_PREFIX)/.")"; return; }
    command -v brew >/dev/null 2>&1 && brew --prefix qt >/dev/null 2>&1 && \
        { brew --prefix qt; return; }
    echo ""
}
QT_PREFIX="$(detect_qt_prefix)"
[[ -n "$QT_PREFIX" ]] || { echo "Error: Qt 6 not found (set CMAKE_PREFIX_PATH)." >&2; exit 1; }
MACDEPLOYQT="$QT_PREFIX/bin/macdeployqt"
[[ -x "$MACDEPLOYQT" ]] || { echo "Error: macdeployqt not at $MACDEPLOYQT" >&2; exit 1; }
echo "→ Qt prefix:   $QT_PREFIX"
echo "→ macdeployqt: $MACDEPLOYQT"

# ── 1. Rust sidecar — same revision as the GUI (version-pinned together) ──────
echo "→ cargo build meeting-server ($PROFILE)…"
REL_FLAG="--release"; [[ "$PROFILE" == debug ]] && REL_FLAG=""
cargo build $REL_FLAG --manifest-path "$RUST_DIR/Cargo.toml" --bin meeting-server
SIDECAR="$RUST_DIR/target/$PROFILE/meeting-server"
[[ -x "$SIDECAR" ]] || { echo "Error: sidecar not built: $SIDECAR" >&2; exit 1; }

# ── 2. Configure + build the .app ────────────────────────────────────────────
echo "→ cmake configure (MA_MACOS_APP_BUNDLE=ON)…"
CMAKE_GEN=(); command -v ninja >/dev/null 2>&1 && CMAKE_GEN=(-G Ninja)
cmake -S "$QT_APP_DIR" -B "$BUILD_DIR" "${CMAKE_GEN[@]}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PREFIX_PATH="$QT_PREFIX" \
    -DMA_MACOS_APP_BUNDLE=ON \
    -DMEETING_SERVER_BIN="$SIDECAR" \
    -DCMAKE_INSTALL_PREFIX="$STAGE"
echo "→ cmake build…"
cmake --build "$BUILD_DIR" --parallel

# ── 3. Install (helper + licenses land INSIDE the .app via CMake rules) ───────
rm -rf "$STAGE"; mkdir -p "$STAGE"
cmake --install "$BUILD_DIR"
APP="$STAGE/MeetingAssistant.app"
[[ -d "$APP" ]] || { echo "Error: $APP not produced by install" >&2; exit 1; }
[[ -f "$APP/Contents/MacOS/meeting-server" ]] || \
    { echo "Error: helper not inside bundle (CMake install rule broken)" >&2; exit 1; }

# ── 4. macdeployqt — Qt runtime only (it skips our helper by design) ──────────
echo "→ macdeployqt (Qt frameworks/plugins/QML)…"
"$MACDEPLOYQT" "$APP" -qmldir="$QT_APP_DIR/qml" -verbose=1

# ── 4b. Prune Qt SQL plugins — UNUSED (persistence is Rust/rusqlite, the app
#        never links Qt SQL). macdeployqt copies them unconditionally and they
#        drag unresolved optional deps (libiodbc/libpq) + a Swift rpath, which
#        is the only source of the macdeployqt ERROR lines. Removing them is
#        correct (never loaded), slims the bundle, and keeps the signed
#        artefact free of dangling-dependency dylibs.
rm -rf "$APP/Contents/PlugIns/sqldrivers"

# ── 4c. Assert the QtMultimedia backend is present BEFORE signing ─────────────
# The in-card audio player needs the ffmpeg media plugin; macdeployqt bundles it
# from the linked Qt6::Multimedia + the QtMultimedia QML import. Catch a missing
# plugin here (dead player on every machine) rather than shipping it. The
# plugin's own dylib and the ffmpeg libs are .dylib under Contents/, so
# codesign-deep.sh's recursive pass already signs them — no signer change needed.
if ! find "$APP/Contents" -name 'libffmpegmediaplugin.dylib' | grep -q .; then
    echo "Error: QtMultimedia ffmpeg plugin not bundled by macdeployqt." >&2
    exit 1
fi
echo "→ QtMultimedia backend bundled ✓"

# ── 4d. Assert the SVG image plugin is present BEFORE signing ────────────────
# MeetyIcon renders the app's button glyphs as data:image/svg+xml through
# QQuick Image. A dev machine can hide a missing bundle plugin by loading it
# from the Qt install, but a clean macOS install cannot. Keep the release
# artefact honest here instead of shipping blank icon buttons.
if ! find "$APP/Contents/PlugIns/imageformats" -name 'libqsvg.dylib' | grep -q .; then
    echo "Error: Qt SVG image plugin not bundled by macdeployqt; button icons will be blank." >&2
    exit 1
fi
echo "→ Qt SVG image plugin bundled ✓"

# ── 4e. Bundle the ffmpeg CLI beside the helper (Contents/MacOS) ──────────────
# Mixed recording (mic via cpal + system via ScreenCaptureKit) shells out to the
# `ffmpeg` CLI, which the sidecar locates next to its own binary. macdeployqt
# bundles only the QtMultimedia *plugin* libs, not the executable, and a clean
# Mac has no ffmpeg on PATH — so mixed mode would fail without this. We build a
# STATIC LGPL ffmpeg from source (mirrors the Linux AppImage's LGPL-from-source
# choice): static libav means a self-contained binary (no dylib/rpath juggling
# inside the bundle), and it lands in Contents/MacOS BEFORE codesign-deep.sh so
# the ad-hoc deep-sign covers it. arm64 needs no x86asm/nasm. Cached in .tools.
FF_VER="6.1.1"
ff_prefix="$TOOLS/ffmpeg-$FF_VER-install"
if [[ ! -x "$ff_prefix/bin/ffmpeg" ]]; then
    echo "→ building LGPL FFmpeg $FF_VER from source (first run; then cached)…"
    mkdir -p "$TOOLS"
    ff_tar="$TOOLS/ffmpeg-$FF_VER.tar.xz"
    [[ -f "$ff_tar" ]] || curl -fL --retry 3 -o "$ff_tar" \
        "https://ffmpeg.org/releases/ffmpeg-$FF_VER.tar.xz"
    ff_src="$TOOLS/ffmpeg-$FF_VER-src"
    rm -rf "$ff_src"; mkdir -p "$ff_src"
    tar -xf "$ff_tar" -C "$ff_src" --strip-components=1
    (
        cd "$ff_src"
        ./configure --prefix="$ff_prefix" \
            --enable-static --disable-shared \
            --disable-ffplay --disable-ffprobe \
            --disable-doc --disable-debug --disable-x86asm
        make -j"$(sysctl -n hw.ncpu)"
        make install
    )
fi
cp "$ff_prefix/bin/ffmpeg" "$APP/Contents/MacOS/ffmpeg"
chmod +x "$APP/Contents/MacOS/ffmpeg"
echo "→ ffmpeg CLI bundled: $APP/Contents/MacOS/ffmpeg"

# ── 5. Ad-hoc deep-sign + verify (the spike's crux) ──────────────────────────
"$SCRIPT_DIR/codesign-deep.sh" "$APP"

# ── 6. Final layout assertion: GUI + sibling helper in Contents/MacOS ────────
echo "→ Bundle layout:"
ls -1 "$APP/Contents/MacOS"
echo "→ Frameworks bundled: $(find "$APP/Contents/Frameworks" -maxdepth 1 -name '*.framework' 2>/dev/null | wc -l | tr -d ' ')"

FINAL="$DIST/MeetingAssistant.app"
rm -rf "$FINAL"; ditto "$APP" "$FINAL"
echo "✓ Signed app: $FINAL"

# ── 7. DMG: drag-to-install layout + in-window first-launch steps ─────────────
# Consumed by BOTH free channels — the Homebrew cask (render-release-cask.sh
# points at this DMG) and the plain "download the DMG" GUI path. The DMG shows a
# soft gradient backdrop with the app + Applications icons in a row near the top
# and the first-launch steps (the un-notarized Gatekeeper "Open Anyway" path)
# printed in the background below them. No separate guide file — the steps are in
# the window itself (see make-dmg-background.swift).
#
# Built with create-dmg. Note: macOS 26 Finder does NOT reliably honor a DMG's
# saved window size, so the window may open larger than requested. The layout
# tolerates this: the gradient stretches cleanly, icons sit up top and text
# below so they never collide, and there is no arrow keyed to an icon position.
# create-dmg is used (not dmgbuild) because its Finder-written background
# reliably renders on macOS 26, whereas dmgbuild's can show blank there.
if [[ $MAKE_DMG -eq 1 ]]; then
    command -v create-dmg >/dev/null 2>&1 || {
        echo "Error: create-dmg not found (brew install create-dmg)." >&2; exit 1; }
    VER="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
        "$FINAL/Contents/Info.plist" 2>/dev/null || echo 0.1.0)"
    DMG="$DIST/MeetingAssistant-$VER.dmg"
    BG="$SCRIPT_DIR/../assets/dmg-background.png"
    [[ -f "$BG" ]] || { echo "Error: missing DMG background: $BG" >&2; exit 1; }
    # Finder treats background pixels as window points; a Retina-style 2x image
    # is cropped instead of scaled and hides the lower first-launch guide.
    BG_W="$(sips -g pixelWidth "$BG" 2>/dev/null | awk '/pixelWidth:/ {print $2}')"
    BG_H="$(sips -g pixelHeight "$BG" 2>/dev/null | awk '/pixelHeight:/ {print $2}')"
    [[ "$BG_W" == 720 && "$BG_H" == 480 ]] || {
        echo "Error: DMG background must be exactly 720x480, got ${BG_W:-?}x${BG_H:-?}." >&2
        exit 1
    }
    echo "→ Building ${DMG} ..."
    rm -f "$DMG"
    DMG_SRC="$(mktemp -d)"
    ditto "$FINAL" "$DMG_SRC/MeetingAssistant.app"
    # Icons in a top row (app left, Applications right); --app-drop-link adds the
    # Applications symlink. Background art paints the steps below the icons.
    create-dmg \
        --volname "Meeting Assistant" \
        --background "$BG" \
        --window-pos 200 120 \
        --window-size 720 480 \
        --icon-size 110 \
        --icon "MeetingAssistant.app" 230 150 \
        --app-drop-link 490 150 \
        --no-internet-enable \
        "$DMG" "$DMG_SRC"
    rm -rf "$DMG_SRC"
    SHA="$(shasum -a 256 "$DMG" | awk '{print $1}')"
    echo "✓ DMG:    $DMG"
    echo "✓ sha256: $SHA"
    echo "  → the Homebrew Cask is generated per-release by"
    echo "    packaging/macos/render-release-cask.sh (no static .rb to edit)."
fi

echo
echo "✓ macOS packaging complete."
echo "  NOTE: spctl rejects this (ad-hoc, un-notarized) — expected. The"
echo "  Homebrew Cask's postflight strips com.apple.quarantine so it launches"
echo "  on a clean Mac. Sunset risk + the durable fix (Developer ID + notarize):"
echo "  packaging/macos/HOMEBREW-SUNSET.md"
