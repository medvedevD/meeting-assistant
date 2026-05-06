import { transcribeFile as transcribeFileApi } from "./api.js";

export function initTranscribe() {
  document.getElementById("btn-transcribe").addEventListener("click", transcribeFile);
}

async function transcribeFile() {
  const path = document.getElementById("filepath").value.trim();
  if (!path) return;

  const btn    = document.getElementById("btn-transcribe");
  const status = document.getElementById("transcribe-status");
  const result = document.getElementById("result");

  btn.disabled = true;
  status.textContent = "Транскрибирование…";
  result.style.display = "none";

  try {
    const text = await transcribeFileApi(path);
    result.textContent = text;
    result.style.display = "block";
    status.textContent = "Готово.";
  } catch (e) {
    status.textContent = "Ошибка: " + e;
  } finally {
    btn.disabled = false;
  }
}
