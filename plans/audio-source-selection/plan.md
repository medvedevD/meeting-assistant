# audio-source-selection

> **Status (2026-06-09): implemented end-to-end, tests green.** Full vertical
> slice landed (core port + adapter enumeration/resolution + `GET
> /api/v1/audio/devices` + settings schema + QML pickers + resolved-name status
> line). macOS system picker is hidden (`system_selectable:false`). Remaining
> work is the "Out" list below (real per-device level meter, hot device-change).
> Linux now enumerates/captures mics via PulseAudio (ADR-5) — ALSA junk gone,
> human device names. Delete this folder once the remaining items are either
> shipped or filed as their own backlog items.

## Problem

Recording captures audio from a *category* (`Mic` / `System` / `Mixed`) but always uses
the **OS default device** within that category — there is no way to pick a specific
microphone or a specific system-output (loopback) source, and the UI never shows which
device is actually being captured ("не понятно какой используется"). Users with multiple
mics / virtual audio devices cannot control or verify the source.

Two distinct gaps:

1. **Selection** — no device enumeration exists anywhere; the user cannot choose a device.
2. **Visibility** — the status line shows the category, not the resolved device name; the
   recording level meter is synthetic (sin-wave math), so it gives no real signal either.

Platform asymmetry that shapes the design:

- **Linux** — system source = a PulseAudio/PipeWire `.monitor` source (selectable).
- **Windows** — system source = a WASAPI output device, loopback (selectable).
- **macOS** — ScreenCaptureKit captures the aggregate system mix; **per-output selection is
  impossible**. Mic selection still applies; system selection must be hidden/disabled.

## Scope

In:
- Device enumeration (mic inputs + system outputs) exposed over the sidecar.
- Per-recording device override + persisted default device in settings.
- Resolve-by-name with default fallback; echo the *resolved* device names to the UI.
- UI: device dropdowns on New Recording + Settings; resolved names on the recording screen.

Out (future):
- Real per-device input-level meter / "test tick" preview (replace synthetic meter).
- Hot device-change handling mid-recording (re-plug while recording).
- Aggregate / multi-mic capture.

## Decisions (ADRs)

- **ADR-1 Identity = native name string + default fallback.** No cross-platform stable ID
  exists (cpal `Device::name()`, pactl source name, WASAPI name). Store the name; match at
  start; if missing, fall back to OS default and report the actual device used.
- **ADR-2 `CaptureSpec` parameter object + `ResolvedDevices` return.** Replaces the growing
  positional arg list on `AudioCapture::start_session`; enables echoing what was opened.
  Resolution moves to *before* the capture thread spawns (surfaces missing-device/permission
  errors at start, like the macOS preflight already does).
- **ADR-3 Separate `AudioDeviceEnumerator` port** (ISP); same adapter implements both.
- **ADR-4 macOS has no system-device selection.** API advertises `system_selectable:false`;
  UI hides/disables the system dropdown.
- **ADR-5 Linux audio goes entirely through PulseAudio/PipeWire, not cpal.** cpal's default
  Linux host is ALSA, which enumerates ~24 pseudo-PCMs (`null`, `pulse`, `hw:`, `plughw:`,
  `front:`, `dsnoop:`, `usbstream:` …) — unusable in a picker. Mics are now enumerated and
  captured via `pactl`/`parec` (the same backend the system leg already used), giving clean
  named sources with human descriptions. `pactl list sources` is run with `LC_ALL=C` so its
  field keys stay English under non-English locales (descriptions remain localized). The mic
  `id`/label are `parse_pulse_sources`-derived (pure, unit-tested). cpal trait imports and
  `record_single` are `cfg(not(linux))`. Windows/macOS keep cpal (clean device names there).

## Deliverables

### Rust — core
- `ports/audio_capture.rs`: add `CaptureSpec`, `ResolvedDevices`; change
  `start_session(&self, session_id, output_path, spec) -> Result<ResolvedDevices, _>`.
- `ports/audio_devices.rs` (new): `AudioDeviceEnumerator` port + `AudioDevice`,
  `AudioDeviceList { input, output, system_selectable }`.
- `ports/mod.rs`: re-export new types.
- `usecases/start_recording.rs`: take device overrides, pass `CaptureSpec`, return resolved
  names alongside the meeting.
- `fakes.rs`: update `FakeAudioCapture` (record spec, return resolved), add
  `FakeAudioDeviceEnumerator`.

### Rust — adapters
- `audio/cpal_capture.rs`: implement enumeration (cpal input devices; pactl `.monitor`
  sources on Linux; WASAPI output devices on Windows; macOS = mic-only, `system_selectable=false`).
  Resolve requested name → device before spawning; fall back to default; return resolved label.
- `audio/mod.rs` / `lib.rs`: export enumerator impl.

### Rust — api
- `routes/recordings.rs`: `StartRequest` gains `mic_device`, `system_device`; response gains
  `resolved { mic, system }`.
- `routes/audio.rs` (new): `GET /api/v1/audio/devices`.
- `router.rs`: register route + enumerator in `AppState`.
- `container.rs`: construct the enumerator adapter.
- `settings_service` / settings schema: `recording.mic_device`, `recording.system_device`.

### Qt/QML
- `qml/AudioDevicesStore.qml` (new): GET `/audio/devices`, expose input/output lists +
  `systemSelectable`, with a manual `refresh()`.
- `qml/screens/NewRecordingScreen.qml`: mic dropdown (source∈mic,mixed), system dropdown
  (source∈system,mixed && systemSelectable); send selected ids on start; show resolved names
  returned from start on the recording status line.
- `qml/screens/settings/RecordingPanel.qml`: same two dropdowns for persisted defaults.
- Reuse `components/MeetyComboBox.qml`.

## Test plan

- core: `start_recording` passes overrides into `CaptureSpec`; default (None) path; resolved
  names propagate. Pure unit tests via fakes.
- adapters: pure tests for name→device resolution + fallback (mirror existing
  `find_monitor_device` pure-fn style); `system_selectable` is false on macOS.
- api: `GET /audio/devices` shape; `POST /recordings` accepts + echoes device fields;
  defaults when omitted.
- Manual: New Recording dropdowns populate, status line shows real device names, unplug →
  falls back to default and the status line reflects it.
