#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

DMG="$TMP_DIR/MeetingAssistant-0.1.0.dmg"
# Render under a Casks/ dir so `brew style` applies the cask cops rather than
# the generic-Ruby ones (Sorbet sigils / frozen-string) that casks are exempt
# from and that would be false positives here.
mkdir -p "$TMP_DIR/Casks"
OUT="$TMP_DIR/Casks/meeting-assistant.rb"
printf 'release cask fixture\n' > "$DMG"
EXPECTED_SHA="$(shasum -a 256 "$DMG" | awk '{print $1}')"

"$SCRIPT_DIR/render-release-cask.sh" \
    v0.1.0-alpha.test \
    "$DMG" \
    "$OUT" \
    example/meeting-assistant

# version = tag without leading v — unique per release so `brew upgrade` sees
# new alpha builds even while the app's marketing version stays 0.1.0.
grep -Fq 'version "0.1.0-alpha.test"' "$OUT"
# url pinned to the literal tag + real DMG name (robust to tag != embedded ver).
grep -Fq 'url "https://github.com/example/meeting-assistant/releases/download/v0.1.0-alpha.test/MeetingAssistant-0.1.0.dmg",' "$OUT"
grep -Fq "sha256 \"$EXPECTED_SHA\"" "$OUT"
grep -Fq 'verified: "github.com/example/meeting-assistant/"' "$OUT"
# the entire point of the free channel: strip quarantine on install.
grep -Fq 'args: ["-dr", "com.apple.quarantine", "#{appdir}/MeetingAssistant.app"]' "$OUT"
grep -Fq 'uninstall quit: "com.meeting-assistant.app"' "$OUT"

# Ruby syntax check (a cask is Ruby).
if command -v ruby >/dev/null 2>&1; then
    ruby -c "$OUT" >/dev/null
fi

# Homebrew style lint when brew is present (local + macOS CI runners have it).
if command -v brew >/dev/null 2>&1; then
    brew style "$OUT" >/dev/null || { echo "Error: brew style rejected the rendered cask." >&2; exit 1; }
fi

echo "macOS release cask renderer test passed"
