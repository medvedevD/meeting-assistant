#!/usr/bin/env bash
# Render a release-pinned macOS installer for an ad-hoc signed alpha DMG.
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
    echo "Usage: render-release-installer.sh <tag> <dmg> <out.sh> [owner/repo]" >&2
    exit 2
fi

TAG="$1"
DMG="$2"
OUT="$3"
REPO="${4:-medvedevD/meeting-assistant}"

[[ "$TAG" == v* ]] || { echo "Error: tag must start with v: $TAG" >&2; exit 1; }
[[ -f "$DMG" ]] || { echo "Error: DMG not found: $DMG" >&2; exit 1; }
[[ "$REPO" == */* ]] || { echo "Error: repo must look like owner/name: $REPO" >&2; exit 1; }

DMG_NAME="$(basename "$DMG")"
SHA_LINE="$(shasum -a 256 "$DMG")"
SHA="${SHA_LINE%% *}"
URL="https://github.com/$REPO/releases/download/$TAG/$DMG_NAME"

mkdir -p "$(dirname "$OUT")"
cat > "$OUT" <<INSTALLER
#!/usr/bin/env bash
# Meeting Assistant $TAG macOS installer.
# Generated from the release DMG; URL and SHA-256 are pinned to this release.
set -euo pipefail

DMG_URL="$URL"
DMG_SHA256="$SHA"
APP_NAME="MeetingAssistant.app"
DEST="/Applications/\$APP_NAME"

if [[ "\$(uname -s)" != "Darwin" ]]; then
    echo "Error: this installer only runs on macOS." >&2
    exit 1
fi

TMP_DIR="\$(mktemp -d)"
DMG_PATH="\$TMP_DIR/$DMG_NAME"
MOUNT_POINT="\$TMP_DIR/mount"
MOUNTED=0

cleanup() {
    if [[ \$MOUNTED -eq 1 ]]; then
        hdiutil detach "\$MOUNT_POINT" -quiet || true
    fi
    rm -rf "\$TMP_DIR"
}
trap cleanup EXIT

echo "Downloading Meeting Assistant $TAG..."
curl -fL --retry 3 -o "\$DMG_PATH" "\$DMG_URL"

ACTUAL_SHA="\$(shasum -a 256 "\$DMG_PATH")"
ACTUAL_SHA="\${ACTUAL_SHA%% *}"
if [[ "\$ACTUAL_SHA" != "\$DMG_SHA256" ]]; then
    echo "Error: DMG checksum mismatch." >&2
    echo "Expected: \$DMG_SHA256" >&2
    echo "Actual:   \$ACTUAL_SHA" >&2
    exit 1
fi

mkdir -p "\$MOUNT_POINT"
hdiutil attach "\$DMG_PATH" -nobrowse -readonly -mountpoint "\$MOUNT_POINT" -quiet
MOUNTED=1

SOURCE="\$MOUNT_POINT/\$APP_NAME"
[[ -d "\$SOURCE" ]] || { echo "Error: \$APP_NAME not found in DMG." >&2; exit 1; }
codesign --verify --deep --strict "\$SOURCE"

install_app() {
    rm -rf "\$DEST"
    ditto "\$SOURCE" "\$DEST"
    xattr -dr com.apple.quarantine "\$DEST"
}

if [[ -w /Applications ]]; then
    install_app
else
    echo "Administrator access is required to install into /Applications."
    sudo rm -rf "\$DEST"
    sudo ditto "\$SOURCE" "\$DEST"
    sudo xattr -dr com.apple.quarantine "\$DEST"
fi

codesign --verify --deep --strict "\$DEST"
if xattr -p com.apple.quarantine "\$DEST" >/dev/null 2>&1; then
    echo "Error: macOS quarantine attribute is still present." >&2
    exit 1
fi

echo "Installed: \$DEST"
open "\$DEST"
INSTALLER

chmod +x "$OUT"
echo "Rendered macOS installer: $OUT"
echo "  tag:    $TAG"
echo "  dmg:    $DMG_NAME"
echo "  sha256: $SHA"
