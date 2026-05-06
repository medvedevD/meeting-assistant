import { useParams } from "react-router-dom";

export default function MeetingDetailPage() {
  const { name } = useParams<{ name: string }>();
  return (
    <div className="flex h-full items-center justify-center">
      <p className="text-[var(--muted-foreground)] text-sm">
        Встреча: {name ?? "не выбрана"} — скоро
      </p>
    </div>
  );
}
