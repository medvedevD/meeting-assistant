import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { Loader2, Download } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { templatesList, generateProtocol, loadProtocol } from "@/api/protocols";
import { queryKeys } from "@/hooks/queryKeys";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { Label } from "@/components/ui/label";
import { downloadMarkdown } from "@/lib/format";

interface ProtocolPanelProps {
  meetingId: string;
  meetingName: string;
}

export function ProtocolPanel({ meetingId, meetingName }: ProtocolPanelProps) {
  const { data: templates = [], isLoading: templatesLoading } = useQuery({
    queryKey: queryKeys.templates,
    queryFn: templatesList,
    staleTime: Infinity,
  });

  const { data: savedProtocol, isLoading: protocolLoading } = useQuery({
    queryKey: queryKeys.protocol(meetingId),
    queryFn: () => loadProtocol(meetingId),
    staleTime: Infinity,
  });

  const [templateName, setTemplateName] = useState<string>("");
  const [markdown, setMarkdown] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);

  const displayMarkdown = markdown ?? savedProtocol?.markdown ?? null;

  async function handleGenerate() {
    setGenerating(true);
    setMarkdown(null);
    try {
      const dto = await generateProtocol({
        meeting_id: meetingId,
        template_name: templateName || undefined,
      });
      setMarkdown(dto.markdown);
      toast.success("Протокол сгенерирован");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setGenerating(false);
    }
  }

  return (
    <div className="space-y-4 p-4">
      {/* Controls */}
      <div className="flex items-end gap-3">
        <div className="flex-1 space-y-1.5">
          <Label htmlFor="template-select" className="text-xs">Шаблон</Label>
          {templatesLoading ? (
            <div className="flex h-9 items-center">
              <Loader2 className="h-4 w-4 animate-spin text-[var(--muted-foreground)]" />
            </div>
          ) : (
            <Select
              id="template-select"
              value={templateName}
              onChange={(e) => setTemplateName(e.target.value)}
            >
              {templates.length === 0 ? (
                <option value="">— нет шаблонов —</option>
              ) : (
                <>
                  <option value="">по умолчанию</option>
                  {templates.map((t) => (
                    <option key={t} value={t}>{t}</option>
                  ))}
                </>
              )}
            </Select>
          )}
        </div>
        <Button size="sm" onClick={handleGenerate} disabled={generating}>
          {generating && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
          {displayMarkdown ? "Перегенерировать" : "Сгенерировать"}
        </Button>
      </div>

      {/* Result */}
      {protocolLoading && !displayMarkdown && (
        <div className="flex items-center gap-2 text-xs text-[var(--muted-foreground)]">
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          Загрузка протокола…
        </div>
      )}
      {displayMarkdown && (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-[var(--muted-foreground)]">
              {markdown ? "Новый протокол" : "Сохранённый протокол"}
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => downloadMarkdown(displayMarkdown, meetingName)}
            >
              <Download className="h-3.5 w-3.5" />
              Скачать .md
            </Button>
          </div>
          <div className="rounded-lg border border-[var(--border)] bg-[var(--card)] p-4">
            <div className="prose prose-sm dark:prose-invert max-w-none text-sm">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{displayMarkdown}</ReactMarkdown>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
