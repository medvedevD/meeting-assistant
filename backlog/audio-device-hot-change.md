# Handle audio device changes during a recording

## Problem

Capture devices are resolved once, before a recording starts. There is no handling for a
device being unplugged or changed while recording.

## Sketch

Detect a lost stream in the capture adapter, keep what has already been recorded, then
either switch to the OS default or stop safely. Report what happened on the recording
status line. Each OS needs its own check: cpal on macOS and Windows, `parec` on Linux.
