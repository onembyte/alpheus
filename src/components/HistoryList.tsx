import type { HistoryEntry } from "../types";
import { fmtDate, fmtKB } from "../format";

const METHOD_LABEL: Record<string, string> = {
  trash: "moved to Trash",
  delete: "deleted",
  command: "command",
};

export default function HistoryList({ entries }: { entries: HistoryEntry[] }) {
  if (entries.length === 0) {
    return (
      <div className="py-16 text-center text-sm text-neutral-500">
        Nothing reclaimed yet — every action lands here with its date and size.
      </div>
    );
  }
  const total = entries.reduce((s, e) => s + e.freed_kb, 0);
  return (
    <div>
      <div className="mb-3 text-[13px] text-neutral-400">
        {fmtKB(total)} reclaimed in total across {entries.length} action
        {entries.length === 1 ? "" : "s"}.
      </div>
      <ul className="divide-y divide-neutral-800/70 rounded-xl border border-neutral-800 bg-neutral-900">
        {[...entries].reverse().map((e, i) => (
          <li key={`${e.timestamp}-${i}`} className="flex items-center gap-3 px-4 py-3">
            <span className="w-32 shrink-0 text-[12px] tabular-nums text-neutral-500">
              {fmtDate(e.timestamp)}
            </span>
            <span className="min-w-0 grow truncate text-[13px] text-neutral-200">{e.title}</span>
            <span className="shrink-0 text-[11px] text-neutral-500">
              {METHOD_LABEL[e.method] ?? e.method}
            </span>
            <span className="w-20 shrink-0 text-right text-[13px] font-semibold tabular-nums text-emerald-300">
              {fmtKB(e.freed_kb)}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
