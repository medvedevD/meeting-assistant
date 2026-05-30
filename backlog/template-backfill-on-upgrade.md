# Template Backfill on Upgrade

## Context

LLM prompt templates live in a user-writable runtime dir
(`prompts/`-equivalent). On first run the bundled set is copied in; the user
can then edit those files and the edits survive upgrades. But if a new app
release **adds** a new bundled template, existing installs do not pick it up —
the install dir is treated as authoritative and the bundle is only consulted
on first run.

## Goal

New bundled templates appear for upgraded installs without overwriting the
user's edits to existing files.

## Sketch

- On startup, list the bundled template files and compare names to what is on
  disk. For each name missing on disk, copy it in. Never touch existing files.
- A small marker file (e.g. `.bundle-version`) is **not** needed because the
  check is by filename — if the user deleted a bundled file deliberately,
  re-introducing it on every upgrade is mildly annoying; mitigate by recording
  deletions in `settings.json` (`removed_bundled_templates: [...]`) and
  honoring that list.

## Expected Outcome

Shipping a new bundled template is a normal release-note item; users get it
on the next upgrade; their customisations and deliberate deletions are
respected.
