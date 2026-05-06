import { useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { CheckCircle, XCircle, Loader2 } from "lucide-react";
import { useSettings, useSettingsMutation } from "@/hooks/useSettings";
import { diagnostics } from "@/api/debug";
import { queryKeys } from "@/hooks/queryKeys";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { AlertDialog } from "@/components/ui/alert-dialog";
import type { SettingsDto, PathInfoDto } from "@/api/types";

// ── PathIndicator ─────────────────────────────────────────────────────────────

function PathIndicator({ info }: { info: PathInfoDto | undefined }) {
  if (!info) return null;
  return info.exists ? (
    <span title={info.writable ? info.path : `Не записываемый: ${info.path}`}>
      <CheckCircle className="h-4 w-4 shrink-0 text-green-500" />
    </span>
  ) : (
    <span title={`Не найдено: ${info.path}`}>
      <XCircle className="h-4 w-4 shrink-0 text-red-500" />
    </span>
  );
}

// ── PathField ────────────────────────────────────────────────────────────────

function PathField({
  label,
  value,
  placeholder,
  indicator,
  onChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  indicator?: PathInfoDto;
  onChange: (v: string) => void;
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      <div className="flex items-center gap-2">
        <Input
          value={value}
          placeholder={placeholder ?? "по умолчанию"}
          onChange={(e) => onChange(e.target.value)}
          className="font-mono text-xs"
        />
        <PathIndicator info={indicator} />
      </div>
    </div>
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function emptyForm(): FormState {
  return {
    model: "",
    db: "",
    recordings: "",
    prompts: "",
    apiKey: "",
    source: "mic",
    echoCancel: false,
    defaultTemplate: "",
  };
}

interface FormState {
  model: string;
  db: string;
  recordings: string;
  prompts: string;
  apiKey: string;
  source: "mic" | "system" | "mixed";
  echoCancel: boolean;
  defaultTemplate: string;
}

function settingsToForm(s: SettingsDto): FormState {
  return {
    model: s.paths.model ?? "",
    db: s.paths.db ?? "",
    recordings: s.paths.recordings ?? "",
    prompts: s.paths.prompts ?? "",
    apiKey: "",
    source: s.recording.source,
    echoCancel: s.recording.echo_cancel,
    defaultTemplate: s.default_template ?? "",
  };
}

function formToDto(form: FormState, current: SettingsDto): SettingsDto {
  return {
    paths: {
      model: form.model.trim() || null,
      db: form.db.trim() || null,
      recordings: form.recordings.trim() || null,
      prompts: form.prompts.trim() || null,
    },
    anthropic_api_key: form.apiKey.trim() || null,
    recording: { source: form.source, echo_cancel: form.echoCancel },
    default_template: form.defaultTemplate.trim() || current.default_template,
  };
}

// ── Settings page ─────────────────────────────────────────────────────────────

export default function SettingsPage() {
  const { data: settings, isLoading } = useSettings();
  const { mutateAsync: save, isPending: isSaving } = useSettingsMutation();

  const { data: diag } = useQuery({
    queryKey: queryKeys.diagnostics,
    queryFn: diagnostics,
    staleTime: 30_000,
  });

  const [form, setForm] = useState<FormState>(emptyForm);
  const [resetOpen, setResetOpen] = useState(false);

  // Populate form whenever server data arrives
  useEffect(() => {
    if (settings) setForm(settingsToForm(settings));
  }, [settings]);

  const isDirty =
    settings !== undefined &&
    JSON.stringify(form) !== JSON.stringify(settingsToForm(settings));

  function patch<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  async function handleSave() {
    if (!settings) return;
    try {
      await save(formToDto(form, settings));
      toast.success("Настройки сохранены");
      const pathsChanged =
        form.model !== (settings.paths.model ?? "") ||
        form.db !== (settings.paths.db ?? "") ||
        form.recordings !== (settings.paths.recordings ?? "") ||
        form.prompts !== (settings.paths.prompts ?? "") ||
        form.apiKey.trim() !== "";
      if (pathsChanged) toast.info("Применится после перезапуска приложения");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  }

  async function handleReset() {
    try {
      const resetDto: SettingsDto = {
        paths: { model: null, db: null, recordings: null, prompts: null },
        anthropic_api_key: null,
        recording: { source: "mic", echo_cancel: false },
        default_template: null,
      };
      await save(resetDto);
      toast.success("Настройки сброшены к значениям по умолчанию");
      toast.info("Применится после перезапуска приложения");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  }

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-5 w-5 animate-spin text-[var(--muted-foreground)]" />
      </div>
    );
  }

  return (
    <>
      <div className="mx-auto max-w-2xl space-y-4 p-6 pb-24">
        <div>
          <h1 className="text-lg font-semibold">Настройки</h1>
          <p className="text-sm text-[var(--muted-foreground)]">
            Конфигурация приложения. Пути применяются после перезапуска.
          </p>
        </div>

        {/* Storage */}
        <Card>
          <CardHeader>
            <CardTitle className="text-sm">Хранилище</CardTitle>
            <CardDescription className="text-xs">
              Пути к файлам. Оставьте пустым для использования значений по умолчанию.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <PathField
              label="Модель Whisper (.bin)"
              value={form.model}
              placeholder={settings?.paths.model ?? undefined}
              indicator={diag?.paths.model}
              onChange={(v) => patch("model", v)}
            />
            <PathField
              label="База данных (.db)"
              value={form.db}
              placeholder={settings?.paths.db ?? undefined}
              indicator={diag?.paths.db}
              onChange={(v) => patch("db", v)}
            />
            <PathField
              label="Папка записей"
              value={form.recordings}
              placeholder={settings?.paths.recordings ?? undefined}
              indicator={diag?.paths.recordings}
              onChange={(v) => patch("recordings", v)}
            />
            <PathField
              label="Папка шаблонов (prompts)"
              value={form.prompts}
              placeholder={settings?.paths.prompts ?? undefined}
              indicator={diag?.paths.prompts}
              onChange={(v) => patch("prompts", v)}
            />
          </CardContent>
        </Card>

        {/* API Key */}
        <Card>
          <CardHeader>
            <CardTitle className="text-sm">Anthropic API ключ</CardTitle>
            <CardDescription className="text-xs">
              Оставьте пустым — текущий ключ не изменится. Пустая строка — очистить.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Input
              type="password"
              value={form.apiKey}
              placeholder="sk-ant-… (оставьте пустым, чтобы не менять)"
              onChange={(e) => patch("apiKey", e.target.value)}
            />
            {diag && (
              <p className="mt-2 flex items-center gap-1.5 text-xs text-[var(--muted-foreground)]">
                {diag.has_anthropic_key ? (
                  <CheckCircle className="h-3.5 w-3.5 text-green-500" />
                ) : (
                  <XCircle className="h-3.5 w-3.5 text-red-500" />
                )}
                {diag.has_anthropic_key ? "Ключ установлен" : "Ключ не установлен"}
              </p>
            )}
          </CardContent>
        </Card>

        {/* Recording defaults */}
        <Card>
          <CardHeader>
            <CardTitle className="text-sm">Запись по умолчанию</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="source-select">Источник звука</Label>
              <Select
                id="source-select"
                value={form.source}
                onChange={(e) =>
                  patch("source", e.target.value as "mic" | "system" | "mixed")
                }
              >
                <option value="mic">Микрофон</option>
                <option value="system">Системный звук</option>
                <option value="mixed">Оба (смешанный)</option>
              </Select>
            </div>
            {(form.source === "system" || form.source === "mixed") && (
              <div className="flex items-center gap-2">
                <input
                  id="echo-cancel"
                  type="checkbox"
                  checked={form.echoCancel}
                  onChange={(e) => patch("echoCancel", e.target.checked)}
                  className="h-4 w-4 rounded border-[var(--input)]"
                />
                <Label htmlFor="echo-cancel">Эхоподавление</Label>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Sticky bottom bar */}
      <div className="fixed bottom-0 left-60 right-0 border-t border-[var(--border)] bg-[var(--background)] px-6 py-3">
        <div className="mx-auto flex max-w-2xl items-center justify-end gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setResetOpen(true)}
            disabled={isSaving}
          >
            Сбросить к default
          </Button>
          <Button size="sm" onClick={handleSave} disabled={!isDirty || isSaving}>
            {isSaving && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            Сохранить
          </Button>
        </div>
      </div>

      <AlertDialog
        open={resetOpen}
        onOpenChange={setResetOpen}
        title="Сбросить настройки?"
        description="Все пути, API ключ и параметры записи будут сброшены к значениям по умолчанию. Это действие нельзя отменить."
        confirmLabel="Сбросить"
        cancelLabel="Отмена"
        variant="destructive"
        onConfirm={handleReset}
      />
    </>
  );
}
