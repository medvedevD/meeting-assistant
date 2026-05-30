# TRACKED RISK — Homebrew unsigned-cask sunset (~September 2026)

**Status:** accepted for v1 · **Owner action required before the date below.**

## The risk

v1 ships macOS with **no paid Apple Developer ID** (locked decision Q3): the
`.app` is ad-hoc signed and **not notarized**. Distribution is a **Homebrew
Cask** that sets `no_quarantine`, which strips the `com.apple.quarantine` bit
so Gatekeeper does not wall off the un-notarized app on a clean machine.

Homebrew is **deprecating support for unsigned / un-notarized casks** on a
rolling basis with a hard cutoff around **September 2026**. After that:

- `homebrew/cask` (the central tap) will **not accept or keep** a cask whose
  artifact is not signed *and* notarized. Our cask cannot live there.
- `no_quarantine` itself is also being curtailed as an escape hatch.

This is dated, not hypothetical. Treat **2026-09-01** as the planning
deadline (Homebrew has signalled "~September 2026"; assume the start of the
month and leave margin).

## What still works after the sunset (the fallback, already built)

A **self-hosted tap** (`packaging/macos/Casks/meeting-assistant.rb`, installed
via `brew tap <owner>/meeting-assistant <repo>` then `brew install --cask`):
homebrew-cask policy does **not** govern third-party taps, so the cask keeps
installing. **But** the fallback is strictly lower-trust and does **not**
satisfy Gatekeeper for an un-notarized app — on a clean machine the user still
hits the Sequoia "Privacy & Security → Open Anyway" wall (the right-click→Open
bypass was removed in macOS 15). `no_quarantine` in a self-hosted cask still
works *for now* but is on the same deprecation track. The self-hosted tap buys
time; it is **not** a permanent answer.

## Documented exit (the real fix)

**Buy an Apple Developer ID ($99/yr) → sign with a Developer ID Application
certificate → enable the hardened runtime → notarize → staple.** This is the
only path that is durable past the sunset and also fixes the *separate*
section-05 problem (ad-hoc signing changes the code-identity hash every build,
so the Screen-Recording TCC grant resets on every update; a stable Developer
ID identity keeps the grant across updates).

This is a **fast-follow, dated**: it must land **before 2026-09-01**, ideally
bundled with the first post-v1 macOS update so existing users stop re-granting
Screen Recording.

The packaging is already staged for this — it is a *configuration* change, not
a redesign:

1. `packaging/macos/codesign-deep.sh`: set `IDENTITY="Developer ID
   Application: <name> (<TEAMID>)"` and add `--options runtime` to every
   `codesign` call. The entitlements file is already in place.
2. Add a notarize + staple step after signing (`xcrun notarytool submit
   --wait` then `xcrun stapler staple`).
3. `packaging/macos/build-app.sh`: notarize/staple the `.dmg` too.
4. Submit the cask to `homebrew/cask` (now eligible) and drop `no_quarantine`;
   keep the self-hosted tap as a mirror.

Until then, v1 is correct and shippable via Homebrew `no_quarantine` + the
self-hosted tap fallback, with the TCC re-prompt accepted (already decided,
section-05).

## Checklist for the owner

- [ ] Before **2026-09-01**: purchase Apple Developer ID.
- [ ] Flip `codesign-deep.sh` to Developer ID + `--options runtime`.
- [ ] Add notarize + staple to `build-app.sh`.
- [ ] Re-submit cask to `homebrew/cask`; drop `no_quarantine`.
- [ ] Ship as a macOS update so the Screen-Recording grant stabilises.
