import type { HistoryEntry } from "../types";
import { fmtDate, fmtKB } from "../format";

const METHOD_LABEL: Record<string, string> = {
  trash: "moved to Trash",
  delete: "deleted",
  command: "command",
};

export default function HistoryPanel({ entries }: { entries: HistoryEntry[] }) {
  if (entries.length === 0) {
    return (
      <div className="glass-card fade-up px-[18px] py-14 text-center">
        <div className="text-[13px]" style={{ color: "var(--txt2)" }}>
          Nothing reclaimed yet
        </div>
        <div className="mono mt-1 text-[10.5px]" style={{ color: "var(--txt3)" }}>
          every action lands here with its date and size
        </div>
      </div>
    );
  }

  const total = entries.reduce((s, e) => s + e.freed_kb, 0);
  return (
    <div className="glass-card fade-up px-[18px] pb-1 pt-4">
      <div className="mb-1.5 flex items-baseline justify-between gap-3">
        <div className="section-label min-w-0 truncate">Every action</div>
        <div
          className="mono flex-none whitespace-nowrap text-[10.5px]"
          style={{ color: "var(--txt3)" }}
        >
          {fmtKB(total)} reclaimed across {entries.length} action{entries.length === 1 ? "" : "s"}
        </div>
      </div>

      <div
        className="hairline-b mono flex items-center gap-3.5 pb-2 pt-1 text-[10px] font-semibold uppercase"
        style={{ color: "var(--txt3)", letterSpacing: ".06em" }}
      >
        <span className="w-[118px] flex-none">When</span>
        <span className="min-w-0 flex-1">Action</span>
        <span className="w-[110px] flex-none text-right">Method</span>
        <span className="w-[76px] flex-none text-right">Freed</span>
      </div>

      {[...entries].reverse().map((e, i) => (
        <div
          key={`${e.timestamp}-${i}`}
          className="hairline-b flex items-center gap-3.5 py-[11px] last:border-0"
        >
          <span className="mono w-[118px] flex-none text-[11px]" style={{ color: "var(--txt3)" }}>
            {fmtDate(e.timestamp)}
          </span>
          <span
            className="min-w-0 flex-1 truncate text-[12.5px] font-medium"
            style={{ color: "var(--txt)" }}
          >
            {e.title}
          </span>
          <span
            className="mono w-[110px] flex-none text-right text-[10.5px]"
            style={{ color: "var(--txt3)" }}
          >
            {e.auto ? "auto · " : ""}
            {METHOD_LABEL[e.method] ?? e.method}
          </span>
          <span
            className="mono w-[76px] flex-none text-right text-[12px] font-semibold"
            style={{ color: "var(--good)" }}
          >
            {fmtKB(e.freed_kb)}
          </span>
        </div>
      ))}
    </div>
  );
}
