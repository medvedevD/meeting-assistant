import { useState } from "react";
import { NavLink } from "react-router-dom";
import { Search, Loader2, FilePlus } from "lucide-react";
import { useMeetings } from "@/hooks/useMeetings";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { formatDateShort } from "@/lib/format";

interface MeetingsListProps {
  onTranscribeFile: () => void;
}

export function MeetingsList({ onTranscribeFile }: MeetingsListProps) {
  const { data: meetings, isLoading, isError } = useMeetings();
  const [search, setSearch] = useState("");

  const filtered = (meetings ?? []).filter((m) =>
    m.name.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="flex h-full flex-col">
      {/* Search */}
      <div className="p-3 border-b border-[var(--border)]">
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--muted-foreground)]" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Поиск встреч…"
            className="pl-8 h-8 text-xs"
          />
        </div>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto">
        {isLoading && (
          <div className="flex items-center justify-center p-8">
            <Loader2 className="h-5 w-5 animate-spin text-[var(--muted-foreground)]" />
          </div>
        )}
        {isError && (
          <p className="p-4 text-xs text-red-500">Ошибка загрузки встреч</p>
        )}
        {!isLoading && !isError && filtered.length === 0 && (
          <div className="flex flex-col items-center justify-center gap-2 p-8 text-center">
            <p className="text-sm text-[var(--muted-foreground)]">
              {search ? "Ничего не найдено" : "Встреч пока нет"}
            </p>
          </div>
        )}
        {filtered.map((m) => (
          <NavLink
            key={m.id}
            to={`/meetings/${m.id}`}
            className={({ isActive }) =>
              cn(
                "block border-b border-[var(--border)] px-3 py-2.5 transition-colors hover:bg-[var(--accent)]",
                isActive && "bg-[var(--accent)]"
              )
            }
          >
            <p className="truncate text-sm font-medium leading-tight">{m.name}</p>
            <div className="mt-0.5 flex items-center justify-between gap-2">
              <span className="text-xs text-[var(--muted-foreground)]">
                {formatDateShort(m.created_at)}
              </span>
              <div className="flex shrink-0 gap-1">
                <Badge variant={m.has_transcript ? "success" : "muted"}>
                  {m.has_transcript ? "Т ✓" : "Т —"}
                </Badge>
                <Badge variant={m.has_protocol ? "success" : "muted"}>
                  {m.has_protocol ? "П ✓" : "П —"}
                </Badge>
              </div>
            </div>
          </NavLink>
        ))}
      </div>

      {/* Footer action */}
      <div className="border-t border-[var(--border)] p-2">
        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-start gap-2 text-xs text-[var(--muted-foreground)]"
          onClick={onTranscribeFile}
        >
          <FilePlus className="h-3.5 w-3.5" />
          + Транскрибировать файл
        </Button>
      </div>
    </div>
  );
}
