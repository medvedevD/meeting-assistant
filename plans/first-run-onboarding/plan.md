# First run and later launches

Phase 3 of [`road-to-1-0`](../road-to-1-0/plan.md).

## Problem

The first-run screen lists three things to set up (permissions, Whisper model, LLM) as
static text. None of them can be done or checked there, and "Start first recording"
marks onboarding as complete whatever the state is. `NewRecordingScreen.qml` has no
check for a transcription model, so a new user can record a whole meeting and only
then get `model_not_selected`, followed by a download, followed by a missing API key.
On later launches nothing re-checks the setup: a deleted model, a removed key or a
permission that macOS dropped after an update all surface as a failed job.

## Scope

**In**

- One source of truth for "is the app ready", served by the sidecar.
- The first-run pane becomes three live steps that can be completed in place.
- A pre-flight line on the recording screen.
- Later launches: a setup notice when something broke, window geometry restored,
  a visible note when a crashed recording was recovered.
- Small fixes found during the audit (listed under Deliverables).

**Out**

- A modal multi-page wizard (see Decisions).
- A bundled sample meeting or demo mode.
- Accounts, telemetry, "what's new" screens.
- The update notice and the stale Screen Recording grant after an update: both belong
  to phase 2 of `road-to-1-0`, but they reuse the setup notice built here.

## Target behaviour

### First launch

1. Splash while the sidecar starts, as today, with user wording instead of
   "Запуск ядра…".
2. The first-run pane keeps its layout (mock-up on the left, text on the right). The
   three bullets become step rows. Each row has a state (to do, in progress, done,
   needs attention) and expands in place:

   | Step | Action in place | Done when |
   |---|---|---|
   | Microphone | "Test microphone": starts the existing level monitor, which makes the OS ask for access, and shows `MeetyLevelMeter`. On macOS the row mentions that recording the other side of a call needs one more permission, asked when system audio is first chosen; onboarding does not ask for it. | Signal seen from the default input. Denied access shows how to re-enable it in system settings. No input device at all is its own state, with import offered instead. |
   | Speech recognition | One recommended model preselected, with its download size and a "Download" button; "Other models" and "I already have a model file" open the Whisper settings. Progress shows in the row and survives leaving the screen. A failed download (no network, no disk space, interrupted) shows the reason and "Retry". | A model is installed and selected, managed or custom. |
   | Minutes | Provider choice, key field, "Check" (the existing settings test). On macOS the row says that saving the key may bring up a keychain prompt. Ollama is the offline option and has its own check: server reachable, then a model pulled, with a hint for each. "Later" is a valid answer: the app is useful for transcripts alone. | The provider check passes, or the user chose "Later". |

3. "Start first recording" is enabled as soon as the microphone step is done. If the
   model is still downloading, the button says so in a caption: recording does not
   need the model, transcription starts when it is ready. Import never depends on
   the microphone step.
4. "Skip setup" completes onboarding without doing the steps. Onboarding is marked
   complete on skip, on the first recording or import, and for a profile that already
   has meetings (as today). It never comes back.
5. One line on the pane states the privacy promise: audio stays on this computer; the
   transcript goes to the chosen provider only when minutes are generated. A second
   line reminds the user to tell participants that the meeting is recorded.
6. The first minutes use a default template, so the path to the first result has no
   template choice. The user can change it on the generate screen as today.
7. On an x86 machine without AVX2 the pane says up front that transcription will not
   work on this computer, instead of the sidecar dying after the meeting.

### macOS system prompts

Rule: a system dialog appears only as the direct result of something the user just
pressed or chose in the app. Nothing at launch, nothing from a background check.

| Prompt | Allowed trigger | Guard |
|---|---|---|
| Microphone | "Test microphone", or starting a recording | Status is read with `QMicrophonePermission`, which does not ask. The level monitor starts only from a button. |
| Screen Recording | Choosing system audio and starting a recording | Already so: `sck_capture::preflight` runs from `start_session`. Onboarding and the launch check never call it. |
| Keychain | Saving a key, "Check", generating minutes | The launch check uses `SecretStore::has_stored`, which does not read the key. |
| Documents folder | Starting the first recording or import | See "To verify": the startup recovery scan may ask at launch today. |

Before a button that will bring up a system dialog, the row says so in one line, so
the dialog is expected.

### Recording screen (every time)

A single pre-flight line above the record button, shown only when something is off:
no model ("the recording will be saved; transcription needs a model" with a download
action), model still downloading, no provider ("minutes are off until you add a
provider"). Nothing blocks recording except a missing microphone.

A meeting recorded before the model was ready shows one action to transcribe it; if
the download finishes while the app is open, transcription starts by itself.

### Later launches

1. Splash, then the app with the window size and position from last time.
2. The setup status is read once per launch and after settings change. If something
   that used to work is broken (model file gone, key missing from the keychain,
   microphone access revoked, and from phase 2 the stale Screen Recording grant), the
   home screen shows a compact notice with one action that fixes it. The first-run
   pane does not return. A locked secret store is its own state ("unlock to generate
   minutes"), not a missing key. Something the user never set up or answered "Later"
   to produces no notice; the reminder for that lives only on the recording screen.
3. If the sidecar recovered a recording after a crash, the home screen says which
   meeting was recovered and links to it.
4. Unfinished jobs resume and appear in the active-jobs indicator, as today.
5. The home state with nothing selected stays, with "New recording" as the main
   action.

## Deliverables

### Sidecar

- `GET /api/v1/setup-status` in `rust/crates/api`: transcription (`ready`,
  `downloading` with progress, `missing`, plus the model id), minutes (`ready`,
  `missing`, `skipped`, `locked`, plus the provider), whether the CPU meets the
  baseline, and the recordings recovered at this start. It must not read a key from
  the OS keyring or open a capture device. Built in `rust/crates/app` from the existing settings and model service. The
  route is additive, so the protocol version does not change. Microphone state is
  not here: the Qt side reads it (see Findings).
- A recommended model per machine class, chosen in the model service from the
  existing catalogue.
- "Minutes later" stored in settings so `skipped` and `missing` can be told apart.
- Recovery reports what it recovered since the last launch so the UI can show it.
- A transcription job submitted with no model waits for it instead of failing, or the
  UI submits it when the model arrives. Pick the smaller change after reading the job
  queue.

### Qt

- `qt-app/qml/SetupStore.qml`: holds the setup status, refreshes on launch, after
  settings are saved and while a download runs. It merges the sidecar's answer with
  the microphone state.
- `qt-app/src/MicrophonePermission.{h,cpp}`: wraps Qt's `QMicrophonePermission`
  (status and request) for QML, next to `ScreenCapturePermission`.
- `WelcomeScreen.qml`: step rows replace `WelcomeBullet`. Move the first-run pane and
  the step rows into their own files; the screen is 571 lines today.
- `NewRecordingScreen.qml`: the pre-flight line.
- A `SetupNotice` component on the home state in `WelcomeScreen.qml`.
- `Main.qml`: save and restore window geometry; splash wording.
- `AppPreferences`: completion on skip.
- `qt-app/i18n`: new strings in both languages.

- A way to force any setup state without a real model or key, for tests and
  screenshots, next to the existing first-run preview switch.

### Delivery order

Four pull requests against `develop`, each useful on its own:

1. `setup-status` route and its contract tests.
2. `SetupStore` and the pre-flight line on the recording screen. This alone ends the
   failure after a recorded meeting.
3. Live steps on the first-run pane, with `MicrophonePermission`.
4. Later launches: setup notice, recovered-recording note, window geometry.

### Fixes found during the audit

- Both "Import" buttons on `WelcomeScreen.qml` call `showNewRecording()`, the same as
  "New recording". Make them open the import flow.
- The `model_missing` hint in `qml/i18n/errors.js` tells the user to fix a file path;
  point it at the download action instead.

### Tests

- `rust/crates/app/tests/sidecar_contract.rs`: `setup-status` on a clean profile, with
  a model installed, with a key stored.
- `qt-app/tests/first_run/`: step states for each status, start button gating, skip,
  notice on a later launch, no return of the first-run pane.
- Manual, on a clean profile on each OS: reach first minutes following only the app;
  deny the microphone and recover; delete the model between launches; break the
  download midway.
- Manual on a packaged macOS build: launch, wait a minute, touch nothing. No system
  dialog may appear. Then note which press produced each dialog.
- There is no telemetry, so the only measure of the flow is watching people: three
  to five users on a clean profile, no hints. Add this to
  `packaging/SMOKE-CHECKLIST.md` for the beta tags.

## Decisions

1. **Live steps on the first-run pane, not a wizard.** A wizard holds the user back
   from the one thing they came for and has to be rebuilt as settings anyway. Steps on
   one pane show everything at once, allow any order, and let the model download run
   while the user does the rest. Confirmed by the owner, 2026-11-14.
2. **Only the microphone gates recording.** The model and the provider are needed
   after the meeting, so a user who installs the app two minutes before a call can
   still record it.
3. **The model download needs a click.** It is hundreds of megabytes and the first
   network use; the recommended model is preselected so it is one click.

4. **System dialogs follow a user action** (owner, 2026-11-14). See "macOS system
   prompts". Screen Recording is therefore not requested during onboarding.
5. **"Later" is silent.** No home-screen notice for something the user chose not to
   set up.
6. **A reminder to inform participants** is shown on the first-run pane.
7. **Default template for the first minutes**, no choice required.

8. **No launch notice for a missing provider key.** The sidecar cannot tell a
   key that was removed from one that was never stored, so a notice would nag
   users who never set minutes up. The recording screen still says it. Launch
   notices cover a recovered recording, a refused microphone that worked
   before, a selected model whose file is gone, and a locked key store.

## Findings from the code

- **A clean profile has no model.** `PersistedTranscriberPrefs::default()` sets
  `model_id: None`, and the first transcription fails with `model_not_selected`.
- **Nothing reads microphone permission today.** The sidecar only learns about it
  when a capture fails. Qt has `QMicrophonePermission` for status and request without
  starting a capture, so the Qt side owns this. It gives a real answer on macOS; on
  Windows and Linux it reports granted, so there the step is done when the test shows
  a signal.
- **Recovery is silent to the UI.** `run_recovery` in
  `adapters/src/audio/recovery.rs` returns a `RecoveryReport`, and no route exposes
  it. Keep the report from startup and return it in `setup-status`.
- **The level test already exists.** `POST /api/v1/audio/monitor` with
  `MeetyLevelMeter` is what the settings screen uses; the microphone step reuses it.

- **The key check can stay quiet.** `SecretStore::has_stored` in
  `adapters/src/secret_store.rs` answers without reading the key from the keyring.
- **Screen Recording is asked on a user action already.** `sck_capture::preflight`
  is called from `start_session` only.
- **Recordings default to the Documents folder** (`app/src/container.rs`), and the
  sidecar scans that folder for recovery before it serves.

## To verify while building

- Whether the startup recovery scan of `~/Documents` brings up the macOS "access
  files in your Documents folder" dialog at launch on a packaged build. If it does,
  it breaks the rule above. Options then: skip the scan until the folder is known to
  exist and to have been used by this app, or give new macOS profiles a default
  outside the protected folders. Existing profiles keep their folder either way.
- Whether `GET /api/v1/settings` and device enumeration at launch reach the keyring
  or the microphone. The settings payload has a `has_key` field; it must come from
  `has_stored`, not from a keyring read.
- The sidecar round-trips a sentinel keyring entry at every start (`probe_keyring`).
  Confirm it stays silent after an update, when the signature has changed.

- That macOS attributes the sidecar's capture to the app bundle, so the status Qt
  reads matches what the sidecar gets. Check on a packaged build, not `run-qt.sh`.
- That `QMicrophonePermission` works in the packaged macOS build without extra
  linking; `NSMicrophoneUsageDescription` is already in `Info.plist.in`.
- Which model to recommend. Proposal: `large-v3-turbo-q5_0` (548 MiB) on Apple
  silicon, `small` (466 MiB) on Windows and Linux while those builds are CPU-only.
  Confirm with `rust/scripts/bench_transcribe.sh` on a slow x86 machine.
