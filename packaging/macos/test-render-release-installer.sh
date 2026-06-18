#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

DMG="$TMP_DIR/MeetingAssistant-0.1.0.dmg"
OUT="$TMP_DIR/install-macos.sh"
printf 'release installer fixture\n' > "$DMG"

"$SCRIPT_DIR/render-release-installer.sh" \
    v0.1.0-alpha.test \
    "$DMG" \
    "$OUT" \
    example/meeting-assistant

bash -n "$OUT"
test -x "$OUT"
grep -Fq \
    'DMG_URL="https://github.com/example/meeting-assistant/releases/download/v0.1.0-alpha.test/MeetingAssistant-0.1.0.dmg"' \
    "$OUT"
grep -Fq 'xattr -dr com.apple.quarantine "$DEST"' "$OUT"
grep -Fq 'xattr -lr "$DEST"' "$OUT"
grep -Fq 'codesign --verify --deep --strict "$DEST"' "$OUT"
grep -Fq 'open "$DEST"' "$OUT"

if grep -Fq 'brew install' "$OUT"; then
    echo "Error: installer must not depend on Homebrew." >&2
    exit 1
fi

echo "macOS release installer renderer test passed"
