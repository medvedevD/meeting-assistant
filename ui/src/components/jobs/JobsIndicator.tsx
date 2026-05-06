import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { jobStatus } from "@/api/jobs";
import { queryKeys } from "@/hooks/queryKeys";
import { cn } from "@/lib/utils";

/** Tracks a specific job id and shows its status */
function JobRow({ jobId }: { jobId: string }) {
  const { data } = useQuery({
    queryKey: queryKeys.job(jobId),
    queryFn: () => jobStatus(jobId),
    refetchInterval: (q) => {
      const s = q.state.data?.status;
      return s === "done" || s === "failed" ? false : 1500;
    },
  });

  if (!data) return null;

  return (
    <div className="flex items-center gap-2 py-1 text-xs">
      {data.status === "running" || data.status === "pending" ? (
        <Loader2 className="h-3 w-3 animate-spin shrink-0 text-[var(--muted-foreground)]" />
      ) : data.status === "done" ? (
        <span className="h-2 w-2 shrink-0 rounded-full bg-green-500" />
      ) : (
        <span className="h-2 w-2 shrink-0 rounded-full bg-red-500" />
      )}
      <span className="truncate text-[var(--muted-foreground)]">
        {data.meeting_id} — {data.status}
      </span>
    </div>
  );
}

interface JobsIndicatorProps {
  activeJobIds: string[];
}

export function JobsIndicator({ activeJobIds }: JobsIndicatorProps) {
  const [open, setOpen] = useState(false);

  if (activeJobIds.length === 0) return null;

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        className={cn(
          "flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-xs transition-colors",
          "hover:bg-[var(--accent)] text-[var(--muted-foreground)]"
        )}
      >
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        <span>{activeJobIds.length} задач активно</span>
      </button>

      {open && (
        <div className="absolute bottom-full left-0 mb-1 w-64 rounded-lg border border-[var(--border)] bg-[var(--card)] p-3 shadow-lg">
          <p className="mb-2 text-xs font-medium">Активные задачи</p>
          {activeJobIds.map((id) => (
            <JobRow key={id} jobId={id} />
          ))}
        </div>
      )}
    </div>
  );
}

/** Self-contained version that tracks jobs from a global store */
export function ActiveJobsTracker() {
  // We don't have a global job store yet — this is a no-op placeholder
  // that keeps the sidebar clean. Jobs in TranscribeFileDialog are self-contained.
  return null;
}
