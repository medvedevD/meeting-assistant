export function initDebug() {
  document.getElementById("btn-diagnostics").addEventListener("click", openDebugWindow);
  document.addEventListener("keydown", (e) => {
    if (e.ctrlKey && e.shiftKey && e.key === "D") {
      e.preventDefault();
      openDebugWindow();
    }
  });
}

async function openDebugWindow() {
  try {
    await window.__TAURI__.core.invoke("show_debug_window");
  } catch (e) {
    console.error("Failed to open debug window:", e);
  }
}
