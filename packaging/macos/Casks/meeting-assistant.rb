# Homebrew Cask for Meeting Assistant (macOS).
#
# v1 is ad-hoc signed and NOT notarized (no paid Apple Developer ID — locked
# decision Q3). `no_quarantine` strips com.apple.quarantine so the un-notarized
# .app launches on a clean machine. This cask is hostable two ways:
#
#   1. homebrew/cask (central) — works ONLY until the ~Sept-2026 unsigned-cask
#      sunset; see ../HOMEBREW-SUNSET.md.
#   2. SELF-HOSTED TAP (fallback, survives the sunset for now):
#        brew tap codemedvedev/meeting-assistant https://github.com/codemedvedev/homebrew-meeting-assistant
#        brew install --cask meeting-assistant
#      Place this file at Casks/meeting-assistant.rb in that tap repo.
#
# Update `version` + `sha256` from `build-app.sh` DMG output each release.
cask "meeting-assistant" do
  version "0.1.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/codemedvedev/meeting-assistant/releases/download/v#{version}/MeetingAssistant-#{version}.dmg",
      verified: "github.com/codemedvedev/meeting-assistant/"
  name "Meeting Assistant"
  desc "Record, transcribe, and generate meeting protocols"
  homepage "https://github.com/codemedvedev/meeting-assistant"

  # Un-notarized: do NOT let Gatekeeper quarantine the app. This is the entire
  # reason the cask exists for v1 (see HOMEBREW-SUNSET.md for the exit).
  auto_updates false
  depends_on macos: ">= :ventura" # macOS 13 floor (ScreenCaptureKit, section-05)

  app "MeetingAssistant.app"

  # `no_quarantine` is expressed via the artifact stanza on modern Homebrew:
  # the app is moved without the quarantine xattr so it launches un-notarized.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine",
                          "#{appdir}/MeetingAssistant.app"],
                   sudo: false
  end

  uninstall quit:   "com.meeting-assistant.app",
            signal: ["TERM", "com.meeting-assistant.app"]

  # Clean uninstall (acceptance criterion). Paths mirror the Rust core's
  # XDG-style layout on macOS (rust/crates/app/src/container.rs +
  # adapters/src/settings_store.rs): data, cache, config trees + the flock.
  zap trash: [
    "~/.local/share/meeting-assistant",
    "~/.cache/meeting-assistant",
    "~/.config/meeting-assistant",
  ]
end
