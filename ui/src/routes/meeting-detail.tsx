import { useState, useEffect } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";
import { FolderOpen, Loader2, FileText } from "lucide-react";
import { useMeetings, useJobSubmit, useJobStatus } from "@/hooks/useMeetings";
import { openPath } from "@/api/debug";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ProtocolPanel } from "@/components/meetings/ProtocolPanel";
import { formatDate } from "@/lib/format";

export default function MeetingDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { data: meetings } = useMeetings();
  const meeting = meetings?.find((m) => m.id === id);

  const { mutateAsync: submitJob, isPending: submitting } = useJobSubmit();
  const [jobId, setJobId] = useState<string | null>(null);

  const { data: job } = useJobStatus(jobId, true);

  // Stop polling when job finishes
  const jobDone = job?.status === "done";
  const jobFailed = job?.status === "failed";

  async function handleTranscribe() {
    if (!meeting) return;
    try {
      const j = await submitJob({ path: meeting.audio_path, name: meeting.name });
      setJobId(j.id);
      toast.info("Транскрибация запущена…");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    if (jobDone && jobId) {
      toast.success("Транскрибация завершена");
      setJobId(null);
    } else if (jobFailed && jobId) {
      toast.error(`Ошибка транскрибации: ${job?.last_error ?? "неизвестно"}`);
      setJobId(null);
    }
  }, [jobDone, jobFailed, jobId, job?.last_error]);

  function openFolder() {
    if (!meeting) return;
    const dir = meeting.audio_path.replace(/\/[^/]+$/, "") || meeting.audio_path;
    openPath(dir).catch(() => {});
  }

  if (!meeting) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-[var(--muted-foreground)]">Выберите встречу из списка</p>
      </div>
    );
  }

  const isTranscribing = submitting || (jobId !== null && !jobDone && !jobFailed);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="border-b border-[var(--border)] px-5 py-4">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h2 className="truncate text-base font-semibold">{meeting.name}</h2>
            <p className="mt-0.5 text-xs text-[var(--muted-foreground)]">
              {formatDate(meeting.created_at)}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Badge variant={meeting.has_transcript ? "success" : "muted"}>
              {meeting.has_transcript ? "транскрипт ✓" : "нет транскрипта"}
            </Badge>
            <Badge variant={meeting.has_protocol ? "success" : "muted"}>
              {meeting.has_protocol ? "протокол ✓" : "нет протокола"}
            </Badge>
            <Button variant="outline" size="sm" onClick={openFolder}>
              <FolderOpen className="h-3.5 w-3.5" />
              Открыть папку
            </Button>
          </div>
        </div>

        {/* Transcribe action */}
        {!meeting.has_transcript && (
          <div className="mt-3 flex items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={handleTranscribe}
              disabled={isTranscribing}
            >
              {isTranscribing && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
              <FileText className="h-3.5 w-3.5" />
              {isTranscribing ? "Транскрибируется…" : "Транскрибировать"}
            </Button>
            {isTranscribing && job && (
              <span className="text-xs text-[var(--muted-foreground)]">
                Статус: {job.status}
              </span>
            )}
          </div>
        )}
      </div>

      {/* Tabs */}
      <Tabs defaultValue="protocols" className="flex-1">
        <TabsList>
          <TabsTrigger value="protocols">Протоколы</TabsTrigger>
          <TabsTrigger value="files">Файлы</TabsTrigger>
        </TabsList>

        <TabsContent value="protocols" className="overflow-y-auto">
          {meeting.has_transcript ? (
            <ProtocolPanel meetingId={meeting.id} meetingName={meeting.name} />
          ) : (
            <div className="flex h-40 items-center justify-center">
              <p className="text-sm text-[var(--muted-foreground)]">
                Сначала нужен транскрипт
              </p>
            </div>
          )}
        </TabsContent>

        <TabsContent value="files" className="overflow-y-auto">
          <div className="space-y-3 p-4">
            <div className="rounded-lg border border-[var(--border)] p-3">
              <p className="mb-1 text-xs font-medium text-[var(--muted-foreground)]">Аудиофайл</p>
              <p className="break-all font-mono text-xs">{meeting.audio_path}</p>
            </div>
            <Button variant="outline" size="sm" onClick={openFolder}>
              <FolderOpen className="h-3.5 w-3.5" />
              Открыть в файл-менеджере
            </Button>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}
