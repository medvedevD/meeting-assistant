#!/usr/bin/env bash
# Render a release-pinned local Homebrew Cask for the macOS alpha DMG.
#
# The app is ad-hoc signed and not notarized in alpha. Installing via this local
# cask lets Homebrew perform the same quarantine-removal postflight that the
# long-lived self-hosted cask uses, without requiring a tap for each prerelease.
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
    echo "Usage: render-release-cask.sh <tag> <dmg> <out.rb> [owner/repo]" >&2
    exit 2
fi

TAG="$1"
DMG="$2"
OUT="$3"
REPO="${4:-codemedvedev/meeting-assistant}"

[[ "$TAG" == v* ]] || { echo "Error: tag must start with v: $TAG" >&2; exit 1; }
[[ -f "$DMG" ]] || { echo "Error: DMG not found: $DMG" >&2; exit 1; }
[[ "$REPO" == */* ]] || { echo "Error: repo must look like owner/name: $REPO" >&2; exit 1; }

VERSION="${TAG#v}"
DMG_NAME="$(basename "$DMG")"
SHA_LINE="$(shasum -a 256 "$DMG")"
SHA="${SHA_LINE%% *}"

mkdir -p "$(dirname "$OUT")"
cat > "$OUT" <<CASK
# Generated for $TAG. Install with:
#   brew install --cask ./$(basename "$OUT")
#
# Alpha is ad-hoc signed and not notarized; the postflight removes the
# GitHub-download quarantine bit after Homebrew installs the app.
cask "meeting-assistant" do
  version "$VERSION"
  sha256 "$SHA"

  url "https://github.com/$REPO/releases/download/$TAG/$DMG_NAME",
      verified: "github.com/$REPO/"
  name "Meeting Assistant"
  desc "Record, transcribe, and generate meeting protocols"
  homepage "https://github.com/$REPO"

  auto_updates false
  depends_on macos: ">= :ventura"

  app "MeetingAssistant.app"

  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine",
                          "#{appdir}/MeetingAssistant.app"],
                   sudo: false
  end

  uninstall quit:   "com.meeting-assistant.app",
            signal: ["TERM", "com.meeting-assistant.app"]

  zap trash: [
    "~/.local/share/meeting-assistant",
    "~/.cache/meeting-assistant",
    "~/.config/meeting-assistant",
  ]
end
CASK

echo "Rendered local cask: $OUT"
echo "  version: $VERSION"
echo "  dmg:     $DMG_NAME"
echo "  sha256:  $SHA"
