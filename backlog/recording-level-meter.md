# Real level meter on the recording screen

## Problem

The meter on the recording screen is decorative. `levelHeight` in
`qt-app/qml/screens/NewRecordingScreen.qml` animates with `Math.sin`, so it moves even
when the microphone captures nothing, and users cannot tell whether sound is being
recorded.

## Sketch

The settings screen already has a real level test (`/api/v1/audio/monitor` and
`MeetyLevelMeter`). Expose the same measurement from the active capture session and bind
the recording screen to it.
