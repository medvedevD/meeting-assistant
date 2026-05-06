import { useState, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { RefreshCw, Copy, Mic, Download, CheckCircle, XCircle, Loader2 } from "lucide-react";
import { diagnostics, diagnosticsLogs, openPath } from "@/api/debug";
import { startRecording, stopRecording } from "@/api/recording";
import { transcribeFile } from "@/api/transcribe";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { PathInfoDto, DeviceInfoDto } from "@/api/types";

// ── Path row ──────────────────────────────────────────────────────────────────

function PathRow({ label, info }: { label: string; info: PathInfoDto }) {
  const sizeMb = info.size_bytes != null ? (info.size_bytes / 1048576).toFixed(1) : null;
  return (
    <tr className="border-b border-[var(--border)] last:border-0">
      <th className="py-1.5 pr-4 text-left text-xs font-medium text-[var(--muted-foreground)] align-top whitespace-nowrap">
        {label}
      </th>
      <td className="py-1.5 text-xs">
        <span className="break-all font-mono text-[var(--foreground)]">{info.path}</span>
        {info.exists ? (
          <span className="ml-2 inline-flex items-center gap-1 text-green-600 dark:text-green-400">
            <CheckCircle className="h-3 w-3" />
            есть{sizeMb && <span className="text-[var(--muted-foreground)]">({sizeMb} МБ)</span>}
            {!info.writable && (
              <span className="text-amber-600 dark:text-amber-400">(не записываем)</span>
            )}
          </span>
        ) : (
          <span className="ml-2 inline-flex items-center gap-1 text-red-500">
            <XCircle className="h-3 w-3" /> отсутствует
          </span>
        )}
        {info.exists && (
          <button
            onClick={() => openPath(info.path).catch(() => {})}
            className="ml-2 text-xs underline text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
          >
            открыть
          </button>
        )}
      </td>
    </tr>
  );
}

// ── Device list ───────────────────────────────────────────────────────────────

function DeviceList({ devices }: { devices: DeviceInfoDto[] }) {
  if (!devices.length) return <p className="text-xs text-[var(--muted-foreground)]">нет устройств</p>;
  return (
    <ul className="space-y-0.5">
      {devices.map((d) => (
        <li key={d.name} className="flex items-center gap-2 text-xs">
          {d.is_default && (
            <span className="rounded bg-[var(--primary)] px-1 py-0.5 text-[10px] text-[var(--primary-foreground)]">
              default
            </span>
          )}
          {d.name.toLowerCase().includes(".monitor") && (
            <span className="rounded bg-[var(--accent)] px-1 py-0.5 text-[10px]">monitor</span>
          )}
          <span className="text-[var(--foreground)]">{d.name}</span>
        </li>
      ))}
    </ul>
  );
}

// ── Mic test ─────────────────────────────────────────────────────────────────

function MicTest() {
  const [result, setResult] = useState<string>("");
  const [busy, setBusy] = useState(false);

  async function runTest() {
    setBusy(true);
    setResult("Запись 3 секунды…");
    let recordingId: string | null = null;
    try {
      const dto = await startRecording({ name: "mic-test", source: "mic", echo_cancel: false });
      recordingId = dto.id;
      setResult("Запись идёт… (3 с)");
      await new Promise((r) => setTimeout(r, 3000));
      setResult("Остановка…");
      const stopped = await stopRecording(recordingId);
      setResult("Транскрибирование…");
      const text = await transcribeFile(stopped.audio_path);
      setResult(text ? `Результат: «${text}»` : "(нет речи)");
    } catch (e) {
      setResult("Ошибка: " + (e instanceof Error ? e.message : String(e)));
      if (recordingId) {
        try { await stopRecording(recordingId); } catch {}
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-2">
      <Button size="sm" variant="outline" onClick={runTest} disabled={busy}>
        {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Mic className="h-3.5 w-3.5" />}
        Тест микрофона (3 с)
      </Button>
      {result && (
        <p className="rounded border border-[var(--border)] bg-[var(--muted)] p-2 font-mono text-xs break-all">
          {result}
        </p>
      )}
    </div>
  );
}

// ── Main DebugApp ─────────────────────────────────────────────────────────────

export function DebugApp() {
  const { data: diag, isLoading, refetch } = useQuery({
    queryKey: ["diagnostics"],
    queryFn: diagnostics,
    staleTime: 15_000,
  });

  const [logs, setLogs] = useState<string[] | null>(null);
  const [logsLoading, setLogsLoading] = useState(false);
  const logsRef = useRef<HTMLPreElement>(null);

  async function refreshLogs() {
    setLogsLoading(true);
    try {
      const lines = await diagnosticsLogs();
      setLogs(lines);
      setTimeout(() => {
        if (logsRef.current) logsRef.current.scrollTop = logsRef.current.scrollHeight;
      }, 50);
    } finally {
      setLogsLoading(false);
    }
  }

  async function copyLogs() {
    const text = (logs ?? diag?.logs ?? []).join("\n");
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
    }
  }

  function exportSnapshot() {
    if (!diag) return;
    const blob = new Blob([JSON.stringify(diag, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "diagnostics.json";
    a.click();
    URL.revokeObjectURL(url);
  }

  const displayLogs = logs ?? diag?.logs ?? [];

  return (
    <div className="min-h-screen bg-[var(--background)] p-5 space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-base font-semibold">Диагностика</h1>
        <div className="flex gap-2">
          <Button size="sm" variant="outline" onClick={() => refetch()}>
            <RefreshCw className="h-3.5 w-3.5" />
            Обновить
          </Button>
          <Button size="sm" variant="outline" onClick={exportSnapshot} disabled={!diag}>
            <Download className="h-3.5 w-3.5" />
            Экспорт JSON
          </Button>
        </div>
      </div>

      {isLoading && (
        <div className="flex items-center gap-2 text-sm text-[var(--muted-foreground)]">
          <Loader2 className="h-4 w-4 animate-spin" />
          Загрузка…
        </div>
      )}

      {diag && (
        <>
          {/* Environment */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">Окружение</CardTitle>
            </CardHeader>
            <CardContent>
              <table className="w-full">
                <tbody>
                  {[
                    ["ОС", diag.os],
                    ["Арх.", diag.arch],
                    ["Версия", diag.app_version],
                    ["cpal host", diag.cpal_host],
                  ].map(([k, v]) => (
                    <tr key={k} className="border-b border-[var(--border)] last:border-0">
                      <th className="py-1.5 pr-4 text-left text-xs font-medium text-[var(--muted-foreground)] whitespace-nowrap">
                        {k}
                      </th>
                      <td className="py-1.5 font-mono text-xs">{v}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </CardContent>
          </Card>

          {/* Paths */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">Пути</CardTitle>
            </CardHeader>
            <CardContent>
              <table className="w-full">
                <tbody>
                  <PathRow label="Модель Whisper" info={diag.paths.model} />
                  <PathRow label="База данных" info={diag.paths.db} />
                  <PathRow label="Записи" info={diag.paths.recordings} />
                  <PathRow label="Шаблоны" info={diag.paths.prompts} />
                </tbody>
              </table>
            </CardContent>
          </Card>

          {/* Dependencies */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">Зависимости</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <div className="flex items-center gap-2 text-xs">
                {diag.ffmpeg_ok ? (
                  <CheckCircle className="h-4 w-4 text-green-500" />
                ) : (
                  <XCircle className="h-4 w-4 text-red-500" />
                )}
                <span>ffmpeg — {diag.ffmpeg_ok ? "найден" : "не найден"}</span>
              </div>
              <div className="flex items-center gap-2 text-xs">
                {diag.has_anthropic_key ? (
                  <CheckCircle className="h-4 w-4 text-green-500" />
                ) : (
                  <XCircle className="h-4 w-4 text-red-500" />
                )}
                <span>Anthropic API key — {diag.has_anthropic_key ? "задан" : "не задан"}</span>
              </div>
            </CardContent>
          </Card>

          {/* Devices */}
          <div className="grid grid-cols-2 gap-4">
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm">Входные устройства</CardTitle>
              </CardHeader>
              <CardContent>
                <DeviceList devices={diag.input_devices} />
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm">Выходные устройства</CardTitle>
              </CardHeader>
              <CardContent>
                <DeviceList devices={diag.output_devices} />
              </CardContent>
            </Card>
          </div>

          {/* Mic test */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">Тест микрофона</CardTitle>
            </CardHeader>
            <CardContent>
              <MicTest />
            </CardContent>
          </Card>

          {/* Logs */}
          <Card>
            <CardHeader className="pb-2">
              <div className="flex items-center justify-between">
                <CardTitle className="text-sm">Логи</CardTitle>
                <div className="flex gap-2">
                  <Button size="sm" variant="outline" onClick={refreshLogs} disabled={logsLoading}>
                    {logsLoading ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <RefreshCw className="h-3.5 w-3.5" />
                    )}
                    Обновить
                  </Button>
                  <Button size="sm" variant="outline" onClick={copyLogs}>
                    <Copy className="h-3.5 w-3.5" />
                    Копировать
                  </Button>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <pre
                ref={logsRef}
                className="h-48 overflow-y-auto rounded border border-[var(--border)] bg-[var(--muted)] p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all"
              >
                {displayLogs.length > 0 ? displayLogs.join("\n") : "(пусто)"}
              </pre>
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}
