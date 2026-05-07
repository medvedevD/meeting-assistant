import { useRef, useState, useEffect } from "react";
import { toast } from "sonner";
import { Loader2, FolderOpen, CheckCircle, XCircle } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { jobSubmit } from "@/api/jobs";
import { jobStatus } from "@/api/jobs";
import { queryKeys } from "@/hooks/queryKeys";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";

interface TranscribeFileDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type JobState =
  | { phase: "idle" }
  | { phase: "running"; jobId: string; status: string }
  | { phase: "done" }
  | { phase: "failed"; error: string };

export function TranscribeFileDialog({ open, onOpenChange }: TranscribeFileDialogProps) {
  const qc = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const [filePath, setFilePath] = useState("");
  const [meetingName, setMeetingName] = useState("");
  const [jobState, setJobState] = useState<JobState>({ phase: "idle" });
  const pollCancelRef = useRef(false);

  function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    // In Tauri webview, File objects expose .path with the full filesystem path
    const tauriPath = (file as File & { path?: string }).path;
    if (tauriPath) {
      setFilePath(tauriPath);
    } else {
      // Fallback: user must type path manually
      setFilePath(file.name);
    }
    if (!meetingName) {
      setMeetingName(file.name.replace(/\.[^.]+$/, ""));
    }
  }

  async function handleSubmit() {
    if (!filePath.trim()) {
      toast.error("Выберите файл");
      return;
    }

    setJobState({ phase: "running", jobId: "", status: "pending" });

    try {
      const job = await jobSubmit({
        path: filePath.trim(),
        name: meetingName.trim() || undefined,
      });

      setJobState({ phase: "running", jobId: job.id, status: job.status });
      pollCancelRef.current = false;

      // Poll until done or failed
      const poll = async () => {
        if (pollCancelRef.current) return;
        try {
          const current = await jobStatus(job.id);
          if (pollCancelRef.current) return;
          if (!current) {
            setJobState({ phase: "failed", error: "Задача не найдена" });
            return;
          }

          if (current.status === "done") {
            setJobState({ phase: "done" });
            toast.success("Транскрибация завершена");
            await qc.invalidateQueries({ queryKey: queryKeys.meetings });
            setTimeout(() => {
              onOpenChange(false);
              resetForm();
            }, 1200);
          } else if (current.status === "failed") {
            setJobState({ phase: "failed", error: current.last_error ?? "неизвестная ошибка" });
          } else {
            setJobState({ phase: "running", jobId: job.id, status: current.status });
            setTimeout(poll, 1500);
          }
        } catch (e) {
          if (pollCancelRef.current) return;
          const msg = e instanceof Error ? e.message : String(e);
          setJobState({ phase: "failed", error: `Ошибка опроса: ${msg}` });
        }
      };

      setTimeout(poll, 1500);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setJobState({ phase: "failed", error: msg });
      toast.error(msg);
    }
  }

  function resetForm() {
    setFilePath("");
    setMeetingName("");
    setJobState({ phase: "idle" });
    if (fileInputRef.current) fileInputRef.current.value = "";
  }

  useEffect(() => {
    if (!open) {
      pollCancelRef.current = true;
    }
  }, [open]);

  function handleClose() {
    pollCancelRef.current = true;
    onOpenChange(false);
    resetForm();
  }

  const isRunning = jobState.phase === "running";

  return (
    <Dialog
      open={open}
      onOpenChange={handleClose}
      title="Транскрибировать файл"
      description="Выберите WAV-файл для транскрибации через Whisper"
    >
      <div className="space-y-4">
        {/* File picker */}
        <div className="space-y-1.5">
          <Label>WAV-файл</Label>
          <div className="flex gap-2">
            <Input
              value={filePath}
              onChange={(e) => setFilePath(e.target.value)}
              placeholder="/path/to/recording.wav"
              className="font-mono text-xs"
              disabled={isRunning}
            />
            <Button
              variant="outline"
              size="icon"
              disabled={isRunning}
              onClick={() => fileInputRef.current?.click()}
              title="Выбрать файл"
            >
              <FolderOpen className="h-4 w-4" />
            </Button>
          </div>
          {/* Hidden native file input */}
          <input
            ref={fileInputRef}
            type="file"
            accept=".wav,audio/wav"
            className="hidden"
            onChange={handleFileChange}
          />
        </div>

        {/* Meeting name */}
        <div className="space-y-1.5">
          <Label>Название встречи (необязательно)</Label>
          <Input
            value={meetingName}
            onChange={(e) => setMeetingName(e.target.value)}
            placeholder="Название по умолчанию из имени файла"
            disabled={isRunning}
          />
        </div>

        {/* Job progress */}
        {jobState.phase === "running" && (
          <div className="flex items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--muted)] px-3 py-2">
            <Loader2 className="h-4 w-4 animate-spin text-[var(--muted-foreground)]" />
            <span className="text-sm text-[var(--muted-foreground)]">
              {jobState.status === "pending" ? "В очереди…" : "Транскрибирование…"}
            </span>
          </div>
        )}
        {jobState.phase === "done" && (
          <div className="flex items-center gap-2 rounded-lg border border-green-200 bg-green-50 px-3 py-2 dark:border-green-900/50 dark:bg-green-950/30">
            <CheckCircle className="h-4 w-4 text-green-600 dark:text-green-400" />
            <span className="text-sm text-green-700 dark:text-green-400">Готово ✓</span>
          </div>
        )}
        {jobState.phase === "failed" && (
          <div className="flex items-center gap-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2 dark:border-red-900/50 dark:bg-red-950/30">
            <XCircle className="h-4 w-4 text-red-500" />
            <span className="text-sm text-red-700 dark:text-red-400">{jobState.error}</span>
          </div>
        )}

        {/* Actions */}
        <div className="flex justify-end gap-2 pt-1">
          <Button variant="outline" size="sm" onClick={handleClose}>
            Отмена
          </Button>
          <Button
            size="sm"
            onClick={handleSubmit}
            disabled={isRunning || !filePath.trim() || jobState.phase === "done"}
          >
            {isRunning && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            Транскрибировать
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
