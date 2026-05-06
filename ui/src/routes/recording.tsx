import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Mic, Square, ArrowRight, AlertCircle } from "lucide-react";
import { useRecording } from "@/hooks/useRecording";
import { useElapsedTimer, formatElapsed } from "@/hooks/useElapsedTimer";
import { useSettings } from "@/hooks/useSettings";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export default function RecordingPage() {
  const navigate = useNavigate();
  const { data: settings } = useSettings();
  const { state, start, stop, reset, isStarting, isStopping, startError, stopError } =
    useRecording();

  const [name, setName] = useState("");
  const [source, setSource] = useState<"mic" | "system" | "mixed">("mic");
  const [echoCancel, setEchoCancel] = useState(false);

  // Pre-fill from settings
  useEffect(() => {
    if (settings) {
      setSource(settings.recording.source);
      setEchoCancel(settings.recording.echo_cancel);
    }
  }, [settings]);

  const startedAt = state.status === "recording" ? state.startedAt : null;
  const elapsed = useElapsedTimer(startedAt);

  const isRecording = state.status === "recording";
  const isStopped = state.status === "stopped";
  const busy = isStarting || isStopping;

  async function handleToggle() {
    if (isRecording) {
      try {
        await stop();
        toast.success("Запись остановлена");
      } catch {
        toast.error("Ошибка при остановке записи");
      }
    } else {
      try {
        await start({ name: name.trim() || undefined, source, echo_cancel: echoCancel });
      } catch {
        // error shown via startError
      }
    }
  }

  const errorMsg =
    startError instanceof Error
      ? startError.message
      : stopError instanceof Error
      ? stopError.message
      : null;

  return (
    <div className="flex h-full flex-col items-center justify-center gap-6 p-8">
      <div className="w-full max-w-md space-y-6">
        {/* Title */}
        <div className="text-center">
          <h1 className="text-lg font-semibold">Запись встречи</h1>
          <p className="text-sm text-[var(--muted-foreground)]">
            Захват микрофона и системного звука
          </p>
        </div>

        {/* Config card — hidden while recording/stopped */}
        {state.status === "idle" && (
          <Card>
            <CardContent className="space-y-4 pt-5">
              <div className="space-y-1.5">
                <Label htmlFor="meeting-name">Название встречи</Label>
                <Input
                  id="meeting-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Например: Иван 1-на-1"
                />
              </div>

              <div className="space-y-1.5">
                <Label htmlFor="source">Источник звука</Label>
                <Select
                  id="source"
                  value={source}
                  onChange={(e) => setSource(e.target.value as typeof source)}
                >
                  <option value="mic">Микрофон</option>
                  <option value="system">Системный звук</option>
                  <option value="mixed">Оба (смешанный)</option>
                </Select>
              </div>

              {(source === "system" || source === "mixed") && (
                <div className="flex items-center gap-2">
                  <input
                    id="echo-cancel"
                    type="checkbox"
                    checked={echoCancel}
                    onChange={(e) => setEchoCancel(e.target.checked)}
                    className="h-4 w-4 rounded border-[var(--input)]"
                  />
                  <Label htmlFor="echo-cancel">Эхоподавление</Label>
                </div>
              )}
            </CardContent>
          </Card>
        )}

        {/* Big record button */}
        {!isStopped && (
          <div className="flex flex-col items-center gap-4">
            <button
              onClick={handleToggle}
              disabled={busy}
              className={cn(
                "relative flex h-28 w-28 items-center justify-center rounded-full transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60",
                isRecording
                  ? "bg-red-500 text-white shadow-lg shadow-red-500/30 hover:bg-red-600"
                  : "bg-[var(--primary)] text-[var(--primary-foreground)] shadow-lg hover:opacity-90"
              )}
            >
              {/* Pulse ring while recording */}
              {isRecording && (
                <span className="absolute inset-0 animate-ping rounded-full bg-red-500 opacity-20" />
              )}
              {isRecording ? (
                <Square className="h-9 w-9" />
              ) : (
                <Mic className="h-9 w-9" />
              )}
            </button>

            {/* Timer / status */}
            {isRecording && (
              <div className="flex items-center gap-2">
                <span className="inline-flex items-center gap-1.5 rounded-full bg-red-100 px-2.5 py-0.5 text-xs font-medium text-red-700 dark:bg-red-900/30 dark:text-red-400">
                  <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-red-500" />
                  REC
                </span>
                <span className="font-mono text-lg font-semibold tabular-nums">
                  {formatElapsed(elapsed)}
                </span>
              </div>
            )}
            {!isRecording && !busy && (
              <p className="text-sm text-[var(--muted-foreground)]">
                Нажмите для начала записи
              </p>
            )}
            {busy && (
              <p className="text-sm text-[var(--muted-foreground)]">
                {isStarting ? "Запуск…" : "Остановка…"}
              </p>
            )}
          </div>
        )}

        {/* Error */}
        {errorMsg && (
          <div className="flex items-start gap-2 rounded-lg border border-red-200 bg-red-50 p-3 dark:border-red-900/50 dark:bg-red-950/30">
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-red-500" />
            <p className="text-sm text-red-700 dark:text-red-400">{errorMsg}</p>
          </div>
        )}

        {/* Post-stop card */}
        {isStopped && (
          <Card className="border-green-200 dark:border-green-900/50">
            <CardContent className="pt-5">
              <div className="flex items-start gap-3">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-green-100 dark:bg-green-900/30">
                  <Mic className="h-4 w-4 text-green-600 dark:text-green-400" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-green-700 dark:text-green-400">
                    Записано ✓
                  </p>
                  <p className="mt-0.5 truncate text-xs text-[var(--muted-foreground)]">
                    {state.dto.name}
                  </p>
                </div>
              </div>
              <div className="mt-4 flex gap-2">
                <Button
                  size="sm"
                  onClick={() => navigate(`/meetings/${state.dto.id}`)}
                >
                  <ArrowRight className="h-3.5 w-3.5" />
                  Перейти к встрече
                </Button>
                <Button size="sm" variant="outline" onClick={reset}>
                  Новая запись
                </Button>
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
