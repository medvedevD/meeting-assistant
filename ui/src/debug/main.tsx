import React from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider } from "@/components/layout/ThemeProvider";
import "../styles/globals.css";

function DebugApp() {
  return (
    <div className="p-6">
      <h1 className="text-lg font-semibold mb-4">Диагностика</h1>
      <p className="text-[var(--muted-foreground)] text-sm">Debug window — скоро (Этап 7)</p>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <DebugApp />
    </ThemeProvider>
  </React.StrictMode>
);
