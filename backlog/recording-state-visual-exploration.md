# Recording State Visual Exploration

## Context

During the Qt redesign, the `NewRecordingScreen` recording state was simplified
to a timer-led layout: recording pill, large timer, meeting name, and stop
action. The earlier large serif `m` glyph was removed because it felt too
decorative for the working app.

## Goal

After the current redesign work is complete, revisit the recording state's
primary visual language and decide whether it should stay timer-led or gain a
more expressive audio-focused element.

## Options To Evaluate

- Keep the timer-led layout as the default production UI.
- Add a restrained pseudo-waveform or level meter.
- Explore a clearer recording mark that is not tied to a large brand glyph.
- Revisit whether any brand-forward glyph belongs in this state at all.

## Expected Outcome

The recording state should feel calm, professional, and immediately legible while
still making it obvious that audio capture is actively running.
