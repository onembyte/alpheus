import type { DiskUsage } from "../types";
import { fmtKB } from "../format";

interface Props {
  usage: DiskUsage | null;
  safeKb: number;
  careKb: number;
  scanning: boolean;
  historyCount: number;
  tab: "cards" | "history";
  onTab: (t: "cards" | "history") => void;
  onRescan: () => void;
}

function freeStatus(freeKb: number): { label: string; cls: string } {
  const gb = freeKb / 1048576;
  if (gb < 10) return { label: "critically low", cls: "text-red-400" };
  if (gb < 15) return { label: "low", cls: "text-amber-400" };
  return { label: "healthy", cls: "text-emerald-400" };
}

export default function DiskHeader(p: Props) {
  const total = p.usage?.total_kb ?? 0;
  const free = p.usage?.free_kb ?? 0;
  const used = Math.max(0, total - free);
  const otherUsed = Math.max(0, used - p.safeKb - p.careKb);
  const pct = (kb: number) => (total > 0 ? `${(kb / total) * 100}%` : "0%");
  const status = freeStatus(free);

  return (
    <header
      data-tauri-drag-region
      className="border-b border-neutral-800/80 bg-neutral-900/60 px-5 pb-4 pt-3"
    >
      {/* row 1 — title (inset for traffic lights) + actions */}
      <div data-tauri-drag-region className="flex items-center gap-3 pl-16">
        <h1 data-tauri-drag-region className="text-sm font-semibold tracking-wide text-neutral-300">
          Storage Manager
        </h1>
        <div className="grow" data-tauri-drag-region />
        <button
          onClick={() => p.onTab(p.tab === "history" ? "cards" : "history")}
          className={`rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors ${
            p.tab === "history"
              ? "border-neutral-500 bg-neutral-700/60 text-neutral-100"
              : "border-neutral-700 bg-neutral-800/60 text-neutral-300 hover:bg-neutral-800"
          }`}
        >
          History{p.historyCount > 0 ? ` (${p.historyCount})` : ""}
        </button>
        <button
          onClick={p.onRescan}
          disabled={p.scanning}
          className="rounded-lg border border-sky-700/60 bg-sky-600/15 px-3 py-1.5 text-xs font-medium text-sky-300 transition-colors hover:bg-sky-600/25 disabled:opacity-50"
        >
          {p.scanning ? "Scanning…" : "Rescan"}
        </button>
      </div>

      {/* row 2 — stat tiles */}
      <div className="mt-4 grid grid-cols-4 gap-3">
        <div className="rounded-xl border border-neutral-800 bg-neutral-900 px-4 py-3">
          <div className="text-[11px] font-medium uppercase tracking-wide text-neutral-500">
            Free
          </div>
          <div className="mt-0.5 text-xl font-semibold tabular-nums">
            {p.usage ? fmtKB(free) : "—"}
          </div>
          <div className={`text-[11px] font-medium ${status.cls}`}>{status.label}</div>
        </div>
        <div className="rounded-xl border border-neutral-800 bg-neutral-900 px-4 py-3">
          <div className="text-[11px] font-medium uppercase tracking-wide text-neutral-500">
            Safe to reclaim
          </div>
          <div className="mt-0.5 text-xl font-semibold tabular-nums">
            {p.safeKb > 0 ? fmtKB(p.safeKb) : "—"}
          </div>
          <div className="text-[11px] text-neutral-500">one click, regenerable</div>
        </div>
        <div className="rounded-xl border border-neutral-800 bg-neutral-900 px-4 py-3">
          <div className="text-[11px] font-medium uppercase tracking-wide text-neutral-500">
            Needs a decision
          </div>
          <div className="mt-0.5 text-xl font-semibold tabular-nums">
            {p.careKb > 0 ? fmtKB(p.careKb) : "—"}
          </div>
          <div className="text-[11px] text-neutral-500">review before removing</div>
        </div>
        <div className="rounded-xl border border-neutral-800 bg-neutral-900 px-4 py-3">
          <div className="text-[11px] font-medium uppercase tracking-wide text-neutral-500">
            Disk used
          </div>
          <div className="mt-0.5 text-xl font-semibold tabular-nums">
            {p.usage ? fmtKB(used) : "—"}
          </div>
          <div className="text-[11px] text-neutral-500">of {p.usage ? fmtKB(total) : "—"}</div>
        </div>
      </div>

      {/* row 3 — usage meter: other used / safe / with-care / free */}
      <div className="mt-3">
        <div className="flex h-2.5 w-full gap-[2px] overflow-hidden rounded-full bg-neutral-800">
          <div className="rounded-full bg-neutral-600" style={{ width: pct(otherUsed) }} />
          {p.safeKb > 0 && (
            <div className="rounded-full bg-emerald-500" style={{ width: pct(p.safeKb) }} />
          )}
          {p.careKb > 0 && (
            <div className="rounded-full bg-amber-500" style={{ width: pct(p.careKb) }} />
          )}
        </div>
        <div className="mt-1.5 flex gap-4 text-[11px] text-neutral-500">
          <span className="inline-flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 rounded-full bg-neutral-600" /> in use
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" /> safe to reclaim
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 rounded-full bg-amber-500" /> needs a decision
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 rounded-full bg-neutral-800 ring-1 ring-neutral-700" />{" "}
            free
          </span>
        </div>
      </div>
    </header>
  );
}
