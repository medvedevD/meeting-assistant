import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import { createBrowserRouter, Navigate, RouterProvider } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import App from "./App";
import RecordingPage from "./routes/recording";
import MeetingsPage from "./routes/meetings";
import MeetingDetailPage from "./routes/meeting-detail";
import SettingsPage from "./routes/settings";
import { ThemeProvider } from "./components/layout/ThemeProvider";
import "./styles/globals.css";

function GlobalHotkeys() {
  useEffect(() => {
    async function handler(e: KeyboardEvent) {
      if (e.ctrlKey && e.shiftKey && e.key === "D") {
        e.preventDefault();
        try {
          const { showDebugWindow } = await import("@/api/debug");
          await showDebugWindow();
        } catch {}
      }
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
  return null;
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 10_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: <Navigate to="/meetings" replace /> },
      { path: "recording", element: <RecordingPage /> },
      {
        path: "meetings",
        element: <MeetingsPage />,
        children: [{ path: ":id", element: <MeetingDetailPage /> }],
      },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <GlobalHotkeys />
        <RouterProvider router={router} />
        <Toaster richColors position="bottom-right" />
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>
);
