import { initRecording } from "./recording.js";
import { initMeetings, loadMeetings } from "./meetings.js";
import { initTranscribe } from "./transcribe.js";
import { initSettings } from "./settings.js";
import { initDebug } from "./debug.js";
import { initJobs } from "./jobs.js";
import { initProtocols } from "./protocols.js";
import { settingsGet } from "./api.js";

// Load persisted recording prefs before initialising the recording card
let recordingPrefs = null;
try {
  const dto = await settingsGet();
  recordingPrefs = dto.recording ?? null;
} catch { /* fall back to localStorage defaults in recording.js */ }

initRecording(recordingPrefs);
initTranscribe();
initMeetings();
await initSettings();
initDebug();
initJobs();
initProtocols();
loadMeetings();
