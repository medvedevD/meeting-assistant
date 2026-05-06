import { Outlet } from "react-router-dom";
import { MeetingsList } from "@/components/meetings/MeetingsList";

export default function MeetingsPage() {
  return (
    <div className="flex h-full">
      {/* Left: list */}
      <div className="flex h-full w-72 shrink-0 flex-col border-r border-[var(--border)]">
        <div className="border-b border-[var(--border)] px-3 py-2.5">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--muted-foreground)]">
            Встречи
          </h2>
        </div>
        <MeetingsList />
      </div>

      {/* Right: detail */}
      <div className="flex-1 overflow-hidden">
        <Outlet />
      </div>
    </div>
  );
}
