import { settingsGet, settingsSet, diagnostics } from "./api.js";

export async function initSettings() {
  document.getElementById("btn-settings-save").addEventListener("click", saveSettings);
  document.getElementById("btn-settings-reset").addEventListener("click", resetSettings);
  await loadSettings();
}

async function loadSettings() {
  const status = document.getElementById("settings-status");
  try {
    const [dto, diag] = await Promise.all([settingsGet(), diagnostics()]);
    populateFields(dto);
    updateIndicators(diag);
  } catch (e) {
    status.textContent = "Не удалось загрузить настройки: " + e;
  }
}

function populateFields(dto) {
  setPlaceholder("settings-model",      dto.paths?.model);
  setPlaceholder("settings-db",         dto.paths?.db);
  setPlaceholder("settings-recordings", dto.paths?.recordings);
  setPlaceholder("settings-prompts",    dto.paths?.prompts);
  // API key: never pre-fill the field, only show placeholder
  document.getElementById("settings-api-key").placeholder =
    "sk-ant-… (оставьте пустым, чтобы не менять)";
}

function setPlaceholder(id, value) {
  const el = document.getElementById(id);
  if (el && value) el.placeholder = value;
}

function updateIndicators(diag) {
  setIndicator("ind-model",      diag?.paths?.model);
  setIndicator("ind-db",         diag?.paths?.db);
  setIndicator("ind-recordings", diag?.paths?.recordings);
  setIndicator("ind-prompts",    diag?.paths?.prompts);
}

function setIndicator(id, pathInfo) {
  const el = document.getElementById(id);
  if (!el || !pathInfo) return;
  if (pathInfo.exists) {
    el.textContent = "✓";
    el.style.color = "#4caf50";
    el.title = pathInfo.path;
  } else {
    el.textContent = "✗";
    el.style.color = "#e94560";
    el.title = "Не найдено: " + pathInfo.path;
  }
}

async function saveSettings() {
  const btn    = document.getElementById("btn-settings-save");
  const status = document.getElementById("settings-status");

  btn.disabled = true;
  status.textContent = "Сохранение…";

  const model      = document.getElementById("settings-model").value.trim() || null;
  const db         = document.getElementById("settings-db").value.trim() || null;
  const recordings = document.getElementById("settings-recordings").value.trim() || null;
  const prompts    = document.getElementById("settings-prompts").value.trim() || null;
  const apiKey     = document.getElementById("settings-api-key").value.trim() || null;

  const pathsChanged = model !== null || db !== null || recordings !== null || prompts !== null;

  try {
    // Read current recording prefs to preserve them
    const current = await settingsGet();
    await settingsSet({
      paths: { model, db, recordings, prompts },
      anthropic_api_key: apiKey,
      recording: current.recording,
      default_template: current.default_template,
    });

    status.textContent = "Сохранено.";
    // Clear inputs after save
    ["settings-model", "settings-db", "settings-recordings", "settings-prompts", "settings-api-key"]
      .forEach(id => { document.getElementById(id).value = ""; });

    await loadSettings();

    if (pathsChanged || apiKey) {
      showToast("Применится после перезапуска приложения");
    }
  } catch (e) {
    status.textContent = "Ошибка: " + e;
  } finally {
    btn.disabled = false;
  }
}

async function resetSettings() {
  const btn    = document.getElementById("btn-settings-reset");
  const status = document.getElementById("settings-status");

  btn.disabled = true;
  status.textContent = "Сброс…";

  try {
    await settingsSet({
      paths: { model: null, db: null, recordings: null, prompts: null },
      anthropic_api_key: null,
      recording: { source: "mic", echo_cancel: false },
      default_template: null,
    });
    status.textContent = "Сброшено к значениям по умолчанию.";
    await loadSettings();
    showToast("Применится после перезапуска приложения");
  } catch (e) {
    status.textContent = "Ошибка: " + e;
  } finally {
    btn.disabled = false;
  }
}

function showToast(msg) {
  const toast = document.getElementById("toast");
  toast.textContent = msg;
  toast.style.display = "block";
  clearTimeout(toast._timer);
  toast._timer = setTimeout(() => { toast.style.display = "none"; }, 3500);
}
