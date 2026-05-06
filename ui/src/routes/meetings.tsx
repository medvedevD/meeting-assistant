import { Outlet } from "react-router-dom";

export default function MeetingsPage() {
  return (
    <div className="flex h-full">
      <div className="flex h-full w-72 shrink-0 flex-col border-r border-[var(--border)] p-4">
        <p className="text-[var(--muted-foreground)] text-sm">Список встреч — скоро</p>
      </div>
      <div className="flex-1 overflow-y-auto">
        <Outlet />
      </div>
    </div>
  );
}
