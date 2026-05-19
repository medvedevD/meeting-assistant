#!/usr/bin/env bash
# Ad-hoc DEEP code-signing for the two-binary MeetingAssistant.app.
#
# Why this exists / what it solves
#   * Apple Silicon refuses to run *unsigned* Mach-O at all — at minimum an
#     ad-hoc signature (`codesign -s -`) is mandatory just to launch.
#   * The bundle has a NESTED helper (meeting-server) plus the Qt frameworks
#     and plugins macdeployqt copied in. macOS verifies signatures
#     INSIDE-OUT: every nested Mach-O must be signed and sealed before the
#     enclosing bundle, or the outer signature is invalid ("code object is
#     not signed at all" / "nested code is modified or invalid").
#   * `codesign --deep` exists but Apple explicitly discourages it (it skips
#     order/seal correctness for helpers). We sign explicitly inside-out
#     instead, which is the section-07-mandated "sign the helper first, then
#     the app" path done thoroughly.
#
# v1 = ad-hoc, NO hardened runtime, NO Apple Developer ID (locked decision).
# The entitlements are embedded but inert until the documented fast-follow
# (buy Developer ID → add `--options runtime` → notarize). See
# packaging/macos/HOMEBREW-SUNSET.md.
#
# Usage: codesign-deep.sh /path/to/MeetingAssistant.app
set -euo pipefail

APP="${1:?usage: codesign-deep.sh /path/to/MeetingAssistant.app}"
SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENTITLEMENTS="$SELF_DIR/entitlements.plist"
IDENTITY="-" # ad-hoc

[[ -d "$APP" ]] || { echo "Error: not a bundle: $APP" >&2; exit 1; }
[[ -f "$ENTITLEMENTS" ]] || { echo "Error: missing $ENTITLEMENTS" >&2; exit 1; }

sign() { # sign <file> [extra codesign args...]
    local f="$1"; shift
    codesign --force --sign "$IDENTITY" --timestamp=none "$@" "$f"
}

echo "→ Ad-hoc deep-signing $APP (inside-out)"

# 1. Frameworks (sign the versioned binary inside each .framework).
while IFS= read -r -d '' fw; do
    base="$(basename "$fw" .framework)"
    bin="$fw/Versions/A/$base"
    [[ -f "$bin" ]] || bin="$fw/$base"
    [[ -f "$bin" ]] && { echo "  framework: $base"; sign "$fw"; }
done < <(find "$APP/Contents/Frameworks" -type d -name '*.framework' -print0 2>/dev/null)

# 2. Loose dylibs + Qt plugins (PlugIns/, and the macdeployqt qml plugin tree).
while IFS= read -r -d '' lib; do
    echo "  dylib: ${lib#"$APP"/}"
    sign "$lib"
done < <(find "$APP/Contents" \( -name '*.dylib' -o -name '*.so' \) -type f -print0 2>/dev/null)

# 3. The nested Rust helper — signed BEFORE the main executable / bundle.
HELPER="$APP/Contents/MacOS/meeting-server"
[[ -f "$HELPER" ]] || { echo "Error: helper missing: $HELPER" >&2; exit 1; }
echo "  helper: meeting-server"
sign "$HELPER" --entitlements "$ENTITLEMENTS"

# 4. The main GUI executable, then 5. the bundle itself — sealed last, with
#    entitlements, so the seal covers every step above.
MAIN="$APP/Contents/MacOS/MeetingAssistant"
[[ -f "$MAIN" ]] || { echo "Error: main exe missing: $MAIN" >&2; exit 1; }
echo "  main: MeetingAssistant"
sign "$MAIN" --entitlements "$ENTITLEMENTS"
echo "  bundle: $(basename "$APP")"
sign "$APP" --entitlements "$ENTITLEMENTS"

echo "→ Verifying signature seal (inside-out)…"
codesign --verify --deep --strict --verbose=2 "$APP"
echo "✓ codesign --verify --deep --strict passed"

echo "→ Gatekeeper assessment (expected to FAIL — ad-hoc, un-notarized):"
if spctl --assess --type execute --verbose=4 "$APP" 2>&1; then
    echo "  (unexpected) spctl accepted the bundle"
else
    echo "  spctl rejected as expected → Homebrew Cask 'no_quarantine' is what"
    echo "  makes this launch on a clean machine (see HOMEBREW-SUNSET.md)."
fi
