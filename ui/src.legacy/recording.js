import { recordingStart, recordingStop, settingsGet, settingsSet } from "./api.js";
import { loadMeetings } from "./meetings.js";

let activeRecordingId = null;

// prefs: { source, echo_cancel } from settings store, or null to use localStorage fallback
export function initRecording(prefs = null) {
  const sourceEl      = document.getElementById("rec-source");
  const echoCancelEl  = document.getElementById("rec-echo-cancel");
  const echoCancelRow = document.getElementById("echo-cancel-row");

  // store prefs take priority; localStorage is the fallback for first-run before GUI-4
  sourceEl.value       = prefs?.source       ?? localStorage.getItem("recording.source")      ?? "mic";
  echoCancelEl.checked = prefs?.echo_cancel  ?? localStorage.getItem("recording.echo_cancel") === "true";
  updateEchoCancelVisibility(sourceEl.value, echoCancelRow);

  sourceEl.addEventListener("change", () => {
    updateEchoCancelVisibility(sourceEl.value, echoCancelRow);
    persistRecordingPrefs(sourceEl.value, echoCancelEl.checked);
  });
  echoCancelEl.addEventListener("change", () => {
    persistRecordingPrefs(sourceEl.value, echoCancelEl.checked);
  });

  document.getElementById("btn-start").addEventListener("click", recordStart);
  document.getElementById("btn-stop").addEventListener("click", recordStop);
}

function updateEchoCancelVisibility(source, row) {
  row.style.display = source === "mixed" ? "flex" : "none";
}

async function persistRecordingPrefs(source, echoCancel) {
  try {
    const current = await settingsGet();
    await settingsSet({ ...current, recording: { source, echo_cancel: echoCancel } });
  } catch {
    // fallback to localStorage if store unavailable
    localStorage.setItem("recording.source", source);
    localStorage.setItem("recording.echo_cancel", String(echoCancel));
  }
}

async function recordStart() {
  const name       = document.getElementById("meeting-name").value.trim() || undefined;
  const source     = document.getElementById("rec-source").value;
  const echoCancel = document.getElementById("rec-echo-cancel").checked;
  const status     = document.getElementById("rec-status");
  const btnStart   = document.getElementById("btn-start");
  const btnStop    = document.getElementById("btn-stop");

  btnStart.disabled = true;
  status.textContent = "Запуск записи…";

  try {
    const dto = await recordingStart({ name, source, echoCancel });
    activeRecordingId = dto.id;

    document.getElementById("recording-badge").classList.add("visible");
    const activeEl = document.getElementById("active-id");
    activeEl.textContent = "ID: " + dto.id;
    activeEl.style.display = "block";

    btnStop.disabled = false;
    status.textContent = "Запись идёт: " + dto.name;
  } catch (e) {
    status.textContent = "Ошибка: " + e;
    btnStart.disabled = false;
  }
}

async function recordStop() {
  if (!activeRecordingId) return;
  const status   = document.getElementById("rec-status");
  const btnStart = document.getElementById("btn-start");
  const btnStop  = document.getElementById("btn-stop");

  btnStop.disabled = true;
  status.textContent = "Остановка…";

  try {
    const dto = await recordingStop(activeRecordingId);
    activeRecordingId = null;

    document.getElementById("recording-badge").classList.remove("visible");
    document.getElementById("active-id").style.display = "none";

    btnStart.disabled = false;
    status.textContent = "Записано: " + dto.audio_path;
    await loadMeetings();
  } catch (e) {
    status.textContent = "Ошибка при остановке: " + e;
    btnStop.disabled = false;
  }
}
