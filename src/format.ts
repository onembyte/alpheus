export function fmtKB(kb: number): string {
  const gb = kb / 1048576;
  if (gb >= 9.95) return `${gb.toFixed(0)} GB`;
  if (gb >= 0.95) return `${gb.toFixed(1)} GB`;
  const mb = kb / 1024;
  if (mb >= 1) return `${mb.toFixed(0)} MB`;
  return `${kb} KB`;
}

export const fmtBytes = (bytes: number): string => fmtKB(bytes / 1024);

export function fmtDate(secs: number): string {
  return new Date(secs * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** `/Users/<name>/…` → `~/…` for compact mono path lines. */
export function shortenPath(p: string): string {
  return p.replace(/^\/Users\/[^/]+/, "~");
}
