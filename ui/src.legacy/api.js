function invoke(cmd, args) {
  return window.__TAURI__.core.invoke(cmd, args);
}

export function recordingStart({ name, source, echoCancel }) {
  return invoke("recording_start", { name, source, echo_cancel: echoCancel });
}
export function recordingStop(id) { return invoke("recording_stop", { id }); }
export function meetingsList() { return invoke("meetings_list"); }
export function transcribeFile(path) { return invoke("transcribe_file", { path }); }

// GUI-3+
export function jobSubmit({ path, name }) { return invoke("job_submit", { path, name }); }
export function jobStatus(id) { return invoke("job_status", { id }); }
export function generateProtocol(args) { return invoke("protocol_generate", args); }
export function templatesList() { return invoke("templates_list"); }
export function settingsGet() { return invoke("settings_get"); }
export function settingsSet(dto) { return invoke("settings_set", { dto }); }
export function diagnostics() { return invoke("diagnostics"); }
export function diagnosticsLogs() { return invoke("diagnostics_logs"); }
export function openPath(path) { return invoke("open_path", { path }); }
