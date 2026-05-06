import { jobStatus } from "./api.js";

export function initJobs() {}

export function startJobPolling(jobId, statusEl, onComplete) {
  const timer = setInterval(async () => {
    try {
      const job = await jobStatus(jobId);
      if (!job) { clearInterval(timer); return; }

      if (job.status === "running") {
        statusEl.textContent = "Транскрибирование…";
      } else if (job.status === "pending") {
        statusEl.textContent = "В очереди…";
      } else {
        clearInterval(timer);
        onComplete(job);
      }
    } catch (e) {
      clearInterval(timer);
      statusEl.textContent = "Ошибка опроса: " + e;
    }
  }, 2000);
}
