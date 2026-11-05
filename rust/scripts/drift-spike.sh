#!/usr/bin/env bash
# section-05 — macOS mic↔system clock-drift spike runner.
#
# Captures macOS *system* audio (ScreenCaptureKit) and the *mic* (cpal)
# simultaneously, then prints how far the two independent audio clocks drift
# over the run. Silence is fine — both backends emit continuous frames.
#
# WHY A SCRIPT: ScreenCaptureKit needs the **Screen Recording** TCC right.
# The grant is attributed to the *responsible GUI app* that launched the
# process. Run this from a plain Terminal.app/iTerm so you can grant
# Screen Recording to the terminal (System Settings → Privacy & Security →
# Screen Recording) without restarting your editor. After toggling the
# permission you must quit & reopen the terminal once for it to take effect.
#
# Usage:
#   ./run-drift-spike.sh            # 60-minute spike (the section-05 spike)
#   ./run-drift-spike.sh 120        # short smoke run (seconds)
set -euo pipefail
cd "$(dirname "$0")"

SECS="${1:-3600}"
echo "Running mic↔system clock-drift spike for ${SECS}s..."
echo "(If macOS denies Screen Recording, the run fails fast with a guided message.)"
echo

MA_DRIFT_SPIKE_SECS="$SECS" cargo test \
  --manifest-path rust/Cargo.toml \
  -p meeting-adapters --lib \
  drift_spike_system_vs_mic -- --ignored --nocapture
