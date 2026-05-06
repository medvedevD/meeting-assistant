import { NavLink } from "react-router-dom";
import { Mic, ListChecks, Settings, Bug } from "lucide-react";
import { ThemeToggle } from "./ThemeToggle";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

const navItems = [
  { to: "/recording", icon: Mic, label: "Запись" },
  { to: "/meetings", icon: ListChecks, label: "Встречи" },
  { to: "/settings", icon: Settings, label: "Настройки" },
];

async function openDebugWindow() {
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const win = await WebviewWindow.getByLabel("debug");
    if (win) {
      await win.show();
      await win.setFocus();
    }
  } catch (e) {
    console.error("Failed to open debug window", e);
  }
}

export function Sidebar() {
  return (
    <aside className="flex h-full w-60 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--card)]">
      {/* Logo */}
      <div className="flex h-14 items-center border-b border-[var(--border)] px-4">
        <span className="text-sm font-semibold tracking-tight text-[var(--foreground)]">
          Meeting Assistant
        </span>
      </div>

      {/* Nav */}
      <nav className="flex flex-1 flex-col gap-0.5 p-2">
        {navItems.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-2.5 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "bg-[var(--accent)] text-[var(--accent-foreground)]"
                  : "text-[var(--muted-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]"
              )
            }
          >
            <Icon className="h-4 w-4" />
            {label}
          </NavLink>
        ))}
      </nav>

      {/* Footer */}
      <div className="border-t border-[var(--border)] p-3 space-y-2">
        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-start gap-2 text-xs text-[var(--muted-foreground)]"
          onClick={openDebugWindow}
        >
          <Bug className="h-3.5 w-3.5" />
          Диагностика
        </Button>
        <div className="flex items-center justify-between px-1">
          <span className="text-xs text-[var(--muted-foreground)]">v0.1.0</span>
          <ThemeToggle />
        </div>
      </div>
    </aside>
  );
}
