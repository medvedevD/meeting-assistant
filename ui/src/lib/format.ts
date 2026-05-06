export function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString("ru-RU", {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDateShort(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString("ru-RU", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  });
}

export function downloadMarkdown(markdown: string, filename: string) {
  const safe = filename.replace(/[^\wА-яЁё\s-]/g, "_");
  const blob = new Blob([markdown], { type: "text/markdown" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${safe}_protocol.md`;
  a.click();
  URL.revokeObjectURL(url);
}
