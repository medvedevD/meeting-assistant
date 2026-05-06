const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);

function esc(s) {
  return String(s)
    .replace(/&/g, "&amp;").replace(/</g, "&lt;")
    .replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

let diagSnapshot = null;

async function load() {
  try {
    diagSnapshot = await invoke("diagnostics");
    renderAll(diagSnapshot);
    document.getElementById("loading").style.display = "none";
    document.getElementById("content").style.display = "block";
  } catch (e) {
    document.getElementById("loading").textContent = "Ошибка загрузки: " + e;
  }
}

function renderAll(d) {
  renderEnv(d);
  renderPaths(d.paths);
  renderDevices(d.input_devices, d.output_devices);
  renderDeps(d);
  renderLogs(d.logs);
}

function renderEnv(d) {
  const rows = [
    ["ОС",           d.os],
    ["Арх.",         d.arch],
    ["Версия",       d.app_version],
    ["cpal host",    d.cpal_host],
  ];
  document.getElementById("env-table").innerHTML =
    rows.map(([k, v]) => `<tr><th>${k}</th><td>${esc(v)}</td></tr>`).join("");
}

function renderPaths(paths) {
  const table = document.getElementById("paths-table");
  const entries = [
    ["Модель Whisper", paths.model],
    ["База данных",    paths.db],
    ["Записи",        paths.recordings],
    ["Шаблоны",       paths.prompts],
  ];
  table.innerHTML = entries.map(([label, info]) => {
    const sizeMb = info.size_bytes != null
      ? ` <span style="color:#888">(${(info.size_bytes / 1048576).toFixed(1)} МБ)</span>`
      : "";
    const status = info.exists
      ? `<span class="ok">✓ есть</span>${sizeMb}${info.writable ? "" : ' <span class="fail">не записываемый</span>'}`
      : `<span class="fail">✗ отсутствует</span>`;
    const openBtn = info.exists
      ? `<button class="secondary" style="margin-left:8px;padding:2px 8px;font-size:0.72rem"
           onclick="openPath(${JSON.stringify(info.path)})">Открыть</button>`
      : "";
    return `<tr>
      <th>${label}</th>
      <td><span style="word-break:break-all">${esc(info.path)}</span>${openBtn}<br/><small>${status}</small></td>
    </tr>`;
  }).join("");
}

function renderDevices(inputs, outputs) {
  document.getElementById("input-devices").innerHTML = deviceList(inputs);
  document.getElementById("output-devices").innerHTML = deviceList(outputs);
}

function deviceList(devices) {
  if (!devices.length) return '<span style="color:#666">нет устройств</span>';
  return devices.map(d => {
    const tags = [];
    if (d.is_default) tags.push('<span class="tag default">default</span>');
    if (d.name.toLowerCase().includes(".monitor"))
      tags.push('<span class="tag monitor">monitor</span>');
    return `<div style="padding:2px 0">${esc(d.name)}${tags.join("")}</div>`;
  }).join("");
}

function renderDeps(d) {
  document.getElementById("ffmpeg-status").innerHTML = d.ffmpeg_ok
    ? '<span class="ok">✓ найден</span>'
    : '<span class="fail">✗ не найден</span>';
  document.getElementById("api-key-status").innerHTML = d.has_anthropic_key
    ? '<span class="ok">✓ задан</span>'
    : '<span class="fail">✗ не задан</span>';
}

function renderLogs(logs) {
  const el = document.getElementById("logs-view");
  el.textContent = logs.length ? logs.join("\n") : "(пусто)";
  el.scrollTop = el.scrollHeight;
}

async function refreshLogs() {
  try {
    const logs = await invoke("diagnostics_logs");
    renderLogs(logs);
    if (diagSnapshot) diagSnapshot.logs = logs;
  } catch (e) {
    document.getElementById("logs-view").textContent = "Ошибка: " + e;
  }
}

async function copyLogs() {
  const text = document.getElementById("logs-view").textContent;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // WebView clipboard fallback
    const ta = document.createElement("textarea");
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    ta.remove();
  }
}

async function openPath(path) {
  try {
    await invoke("open_path", { path });
  } catch (e) {
    console.error("open_path failed:", e);
  }
}

window.openPath = openPath;

async function runMicTest() {
  const btn = document.getElementById("btn-mic-test");
  const result = document.getElementById("mic-test-result");

  btn.disabled = true;
  result.textContent = "Запись 3 секунды…";

  let recordingId = null;
  try {
    const dto = await invoke("recording_start", { name: "mic-test", source: "mic", echo_cancel: false });
    recordingId = dto.id;
    result.textContent = "Запись идёт… (3 с)";

    await new Promise(r => setTimeout(r, 3000));

    result.textContent = "Остановка…";
    const stopped = await invoke("recording_stop", { id: recordingId });

    result.textContent = "Транскрибирование…";
    const text = await invoke("transcribe_file", { path: stopped.audio_path });
    result.textContent = text ? `Результат: «${text}»` : "(нет речи)";
  } catch (e) {
    result.textContent = "Ошибка: " + e;
    if (recordingId) {
      try { await invoke("recording_stop", { id: recordingId }); } catch {}
    }
  } finally {
    btn.disabled = false;
  }
}

function exportSnapshot() {
  if (!diagSnapshot) return;
  const safe = { ...diagSnapshot, has_anthropic_key: diagSnapshot.has_anthropic_key };
  const json = JSON.stringify(safe, null, 2);
  const blob = new Blob([json], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = "diagnostics.json";
  a.click();
  URL.revokeObjectURL(url);
}

document.addEventListener("DOMContentLoaded", () => {
  load();
  document.getElementById("btn-refresh-logs").addEventListener("click", refreshLogs);
  document.getElementById("btn-copy-logs").addEventListener("click", copyLogs);
  document.getElementById("btn-mic-test").addEventListener("click", runMicTest);
  document.getElementById("btn-export").addEventListener("click", exportSnapshot);
});
