import { meetingsList, jobSubmit, openPath } from "./api.js";
import { startJobPolling } from "./jobs.js";
import { openProtocolModal } from "./protocols.js";

export function initMeetings() {
  document.getElementById("btn-refresh-meetings").addEventListener("click", loadMeetings);
}

export async function loadMeetings() {
  const container = document.getElementById("meetings-list");
  const empty     = document.getElementById("meetings-empty");
  try {
    const meetings = await meetingsList();
    container.innerHTML = "";
    if (meetings.length === 0) {
      empty.textContent = "Встреч пока нет.";
      container.appendChild(empty);
      return;
    }
    for (const m of meetings) {
      container.appendChild(buildMeetingItem(m));
    }
  } catch (e) {
    empty.textContent = "Ошибка загрузки: " + e;
    container.innerHTML = "";
    container.appendChild(empty);
  }
}

function buildMeetingItem(m) {
  const item = document.createElement("div");
  item.className = "meeting-item";

  // Name + badge row
  const topRow = document.createElement("div");
  topRow.style.cssText = "display:flex;align-items:center;gap:10px";

  const nameEl = document.createElement("div");
  nameEl.className = "meeting-name";
  nameEl.textContent = m.name;

  const badge = document.createElement("span");
  badge.className = "meeting-badge" + (m.has_transcript ? "" : " no");
  badge.textContent = m.has_transcript ? "транскрипт ✓" : "без транскрипта";

  topRow.appendChild(nameEl);
  topRow.appendChild(badge);

  // Path
  const pathEl = document.createElement("div");
  pathEl.className = "meeting-path";
  pathEl.textContent = m.audio_path;

  // Job progress line (hidden until needed)
  const statusEl = document.createElement("div");
  statusEl.className = "status-line";
  statusEl.style.display = "none";

  // Action buttons
  const actionsRow = document.createElement("div");
  actionsRow.className = "meeting-actions";

  if (!m.has_transcript) {
    const btnT = smallBtn("Транскрибировать", "");
    btnT.addEventListener("click", async () => {
      btnT.disabled = true;
      statusEl.style.display = "block";
      statusEl.textContent = "Создание задачи…";
      try {
        const job = await jobSubmit({ path: m.audio_path, name: m.name });
        statusEl.textContent = "В очереди…";
        startJobPolling(job.id, statusEl, async (finished) => {
          if (finished.status === "succeeded") {
            statusEl.textContent = "Готово ✓";
            await loadMeetings();
          } else {
            statusEl.textContent = "Ошибка: " + (finished.last_error ?? "неизвестно");
            btnT.disabled = false;
          }
        });
      } catch (e) {
        statusEl.textContent = "Ошибка: " + e;
        btnT.disabled = false;
      }
    });
    actionsRow.appendChild(btnT);
  } else {
    const btnP = smallBtn("Протокол", "");
    btnP.addEventListener("click", () => openProtocolModal(m.id, m.name));
    actionsRow.appendChild(btnP);
  }

  const btnOpen = smallBtn("Открыть папку", "secondary");
  btnOpen.addEventListener("click", () => {
    // strip filename to get directory
    const dir = m.audio_path.includes("/")
      ? m.audio_path.replace(/\/[^/]+$/, "")
      : m.audio_path;
    openPath(dir);
  });
  actionsRow.appendChild(btnOpen);

  item.appendChild(topRow);
  item.appendChild(pathEl);
  item.appendChild(statusEl);
  item.appendChild(actionsRow);
  return item;
}

function smallBtn(text, cls) {
  const btn = document.createElement("button");
  btn.textContent = text;
  if (cls) btn.className = cls;
  return btn;
}
