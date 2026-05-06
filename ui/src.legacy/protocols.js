import { templatesList, generateProtocol } from "./api.js";

let currentMeetingId   = null;
let currentMeetingName = null;
let currentMarkdown    = null;

export function initProtocols() {
  document.getElementById("btn-modal-close").addEventListener("click", closeModal);
  document.getElementById("btn-modal-generate").addEventListener("click", generate);
  document.getElementById("btn-modal-download").addEventListener("click", download);
  // click on backdrop closes modal
  document.getElementById("protocol-modal").addEventListener("click", (e) => {
    if (e.target === e.currentTarget) closeModal();
  });
}

export async function openProtocolModal(meetingId, meetingName) {
  currentMeetingId   = meetingId;
  currentMeetingName = meetingName;
  currentMarkdown    = null;

  document.getElementById("modal-title").textContent    = `Протокол: ${meetingName}`;
  document.getElementById("modal-status").textContent   = "";
  document.getElementById("modal-result").style.display = "none";
  document.getElementById("modal-result").textContent   = "";
  document.getElementById("modal-actions").style.display = "none";
  document.getElementById("protocol-modal").style.display = "flex";

  const sel = document.getElementById("modal-template");
  sel.innerHTML = '<option value="">Загрузка…</option>';
  try {
    const names = await templatesList();
    sel.innerHTML = names.length
      ? names.map(n => `<option value="${n}">${n}</option>`).join("")
      : '<option value="">— нет шаблонов —</option>';
  } catch {
    sel.innerHTML = '<option value="">Ошибка загрузки</option>';
  }
}

function closeModal() {
  document.getElementById("protocol-modal").style.display = "none";
}

async function generate() {
  const btn          = document.getElementById("btn-modal-generate");
  const status       = document.getElementById("modal-status");
  const templateName = document.getElementById("modal-template").value || null;

  btn.disabled = true;
  status.textContent = "Генерация протокола…";
  document.getElementById("modal-result").style.display  = "none";
  document.getElementById("modal-actions").style.display = "none";

  try {
    const dto = await generateProtocol({
      meeting_id:    currentMeetingId,
      template_name: templateName,
    });
    currentMarkdown = dto.markdown;

    const resultEl = document.getElementById("modal-result");
    resultEl.textContent = currentMarkdown;
    resultEl.style.display = "block";
    document.getElementById("modal-actions").style.display = "block";
    status.textContent = "";
  } catch (e) {
    status.textContent = "Ошибка: " + e;
  } finally {
    btn.disabled = false;
  }
}

function download() {
  if (!currentMarkdown) return;
  const safeName = currentMeetingName.replace(/[^\wЀ-ӿ-]/g, "_");
  const blob = new Blob([currentMarkdown], { type: "text/markdown" });
  const url  = URL.createObjectURL(blob);
  const a    = document.createElement("a");
  a.href     = url;
  a.download = `${safeName}_protocol.md`;
  a.click();
  URL.revokeObjectURL(url);
}
