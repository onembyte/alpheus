import type { Card, DiskUsage } from "../types";
import { cardColor, OTHER_COLOR } from "../tiers";
import { fmtKB } from "../format";

interface Props {
  usage: DiskUsage;
  cards: Card[];
  onFreeUp: () => void;
}

interface Seg {
  key: string;
  label: string;
  color: string;
  kb: number;
}

/**
 * One series feeds both the ring and the capacity bar: everything-else on
 * disk (muted), then reclaim categories by size, free space as the gap.
 */
export function buildSeries(usage: DiskUsage, cards: Card[]): { segs: Seg[]; restKb: number } {
  const action = cards.filter((c) => c.action !== "explain");
  const top = action.slice(0, 7);
  const restKb = action.slice(7).reduce((s, c) => s + c.size_kb, 0);
  const reclaimable = action.reduce((s, c) => s + c.size_kb, 0);
  const usedKb = Math.max(0, usage.total_kb - usage.free_kb);
  const otherKb = Math.max(0, usedKb - reclaimable);

  const segs: Seg[] = [
    { key: "other", label: "Everything else", color: OTHER_COLOR, kb: otherKb },
    ...top.map((c, i) => ({ key: c.id, label: c.title, color: cardColor(i), kb: c.size_kb })),
  ];
  if (restKb > 0) {
    segs.push({ key: "rest", label: "Other categories", color: cardColor(7), kb: restKb });
  }
  return { segs, restKb };
}

const RING_C = 628.3; // 2π · r100

export default function OverviewPanel({ usage, cards, onFreeUp }: Props) {
  const freeGb = usage.free_kb / 1048576;
  const freePct = usage.total_kb > 0 ? (usage.free_kb / usage.total_kb) * 100 : 0;
  const { segs } = buildSeries(usage, cards);
  const reclaimable = cards
    .filter((c) => c.action !== "explain")
    .reduce((s, c) => s + c.size_kb, 0);
  const safeKb = cards
    .filter((c) => c.tier === "safe" && c.action !== "explain")
    .reduce((s, c) => s + c.size_kb, 0);

  const low = freeGb < 15;
  const bannerColor = freeGb < 10 ? "var(--danger)" : "var(--warn)";
  const statusColor = freeGb < 10 ? "var(--danger)" : freeGb < 15 ? "var(--warn)" : "var(--good)";

  let acc = 0;
  const ringSegs = segs.map((s) => {
    const len = usage.total_kb > 0 ? (s.kb / usage.total_kb) * RING_C : 0;
    const seg = { ...s, len, offset: -acc };
    acc += len;
    return seg;
  });

  return (
    <div className="fade-up flex flex-col gap-4">
      {low && (
        <div
          className="flex items-center gap-3.5 rounded-[15px] px-4 py-3"
          style={{
            background: `linear-gradient(120deg, color-mix(in srgb, ${bannerColor} 20%, transparent), color-mix(in srgb, ${bannerColor} 6%, transparent))`,
            boxShadow: `inset 0 1px 0 rgba(255,255,255,.35), 0 0 0 1px color-mix(in srgb, ${bannerColor} 30%, transparent)`,
          }}
        >
          <svg
            width="19"
            height="19"
            viewBox="0 0 20 20"
            fill="none"
            style={{ color: bannerColor, flex: "none" }}
          >
            <path
              d="M10 2.6 18.2 17H1.8L10 2.6Z"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinejoin="round"
            />
            <path
              d="M10 7.6v4.2M10 14.4v.1"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
            />
          </svg>
          <div className="flex-1">
            <div className="text-[12.5px] font-semibold" style={{ color: "var(--txt)" }}>
              Startup disk almost full — {fmtKB(usage.free_kb)} available ({freePct.toFixed(2)}%)
            </div>
            <div className="mono text-[11px]" style={{ color: "var(--txt2)" }}>
              {fmtKB(reclaimable)} reclaimable · {fmtKB(safeKb)} safe one-click · no restart needed
            </div>
          </div>
          <button
            onClick={onFreeUp}
            className="glass-chip btn-focus h-7 flex-none rounded-lg px-3.5 text-[11.5px] font-semibold"
            style={{ color: "var(--txt)" }}
          >
            Free up space
          </button>
        </div>
      )}

      <div className="flex items-stretch gap-4">
        {/* allocation ring */}
        <div className="glass-card flex w-[300px] flex-none flex-col items-center p-[18px]">
          <div className="section-label self-start" style={{ marginBottom: 6 }}>
            Allocation map
          </div>
          <div className="relative h-[218px] w-[218px]">
            <svg
              className="ring-in"
              width="218"
              height="218"
              viewBox="0 0 240 240"
              style={{ transformOrigin: "109px 109px" }}
            >
              <g transform="rotate(-90 120 120)" fill="none">
                <circle cx="120" cy="120" r="100" stroke="var(--track)" strokeWidth="25" />
                {ringSegs.map((s) => (
                  <circle
                    key={s.key}
                    cx="120"
                    cy="120"
                    r="100"
                    stroke={s.color}
                    strokeWidth="25"
                    strokeDasharray={`${Math.max(0, s.len - 1.5)} ${RING_C}`}
                    strokeDashoffset={s.offset}
                  />
                ))}
              </g>
            </svg>
            <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-0.5">
              <div
                className="text-[24px] font-semibold"
                style={{ color: "var(--txt)", letterSpacing: "-.02em", fontVariantNumeric: "tabular-nums" }}
              >
                {fmtKB(usage.free_kb)}
              </div>
              <div className="mono text-[10.5px]" style={{ color: "var(--txt3)" }}>
                free of {fmtKB(usage.total_kb)}
              </div>
              <div
                className="mono mt-1.5 rounded-[7px] px-2 py-[3px] text-[10.5px] font-semibold"
                style={{
                  color: statusColor,
                  background: `color-mix(in srgb, ${statusColor} 16%, transparent)`,
                  boxShadow: `0 0 0 1px color-mix(in srgb, ${statusColor} 28%, transparent)`,
                }}
              >
                {freePct.toFixed(2)}% FREE
              </div>
            </div>
          </div>
          <div
            className="mono mt-2.5 flex w-full justify-between text-[10.5px]"
            style={{ color: "var(--txt3)" }}
          >
            <span>arcs: what's reclaimable</span>
            <span>gap: free space</span>
          </div>
        </div>

        {/* capacity bar + legend */}
        <div className="glass-card min-w-0 flex-1 p-[18px]">
          <div className="mb-3 flex items-baseline justify-between gap-3">
            <div className="section-label">Capacity</div>
            <div className="mono text-[10px]" style={{ color: "var(--txt3)" }}>
              {fmtKB(reclaimable)} reclaimable · {segs.length - 1} categories
            </div>
          </div>

          <div
            className="relative flex h-[26px] gap-[2px] overflow-hidden rounded-[9px]"
            style={{ background: "var(--track)", boxShadow: "inset 0 1px 3px rgba(0,0,0,.22)" }}
          >
            {segs.map(
              (s, i) =>
                s.kb > 0 && (
                  <div
                    key={s.key}
                    className="bar-in"
                    style={{
                      width: `${(s.kb / usage.total_kb) * 100}%`,
                      background: s.color,
                      animationDelay: `${i * 0.05}s`,
                    }}
                  />
                ),
            )}
            <div className="flex-1" />
            <div
              className="pointer-events-none absolute inset-0"
              style={{
                background:
                  "linear-gradient(180deg, rgba(255,255,255,.42), rgba(255,255,255,.06) 48%, rgba(0,0,0,.10))",
              }}
            />
          </div>

          <div className="mt-3.5 grid grid-cols-2 gap-x-3.5 gap-y-2.5">
            {segs.map((s) => (
              <div key={s.key} className="flex items-center gap-[7px]">
                <div
                  className="h-2 w-2 flex-none rounded-[3px]"
                  style={{ background: s.color }}
                />
                <div className="min-w-0">
                  <div
                    className="truncate text-[11.5px] font-medium"
                    style={{ color: "var(--txt)" }}
                  >
                    {s.label}
                  </div>
                  <div className="mono text-[10px]" style={{ color: "var(--txt3)" }}>
                    {fmtKB(s.kb)}
                  </div>
                </div>
              </div>
            ))}
            <div className="flex items-center gap-[7px]">
              <div
                className="h-2 w-2 flex-none rounded-[3px]"
                style={{ background: "var(--track)", boxShadow: "0 0 0 1px var(--edge-lo)" }}
              />
              <div className="min-w-0">
                <div className="truncate text-[11.5px] font-medium" style={{ color: "var(--txt2)" }}>
                  Available
                </div>
                <div className="mono text-[10px]" style={{ color: "var(--txt3)" }}>
                  {fmtKB(usage.free_kb)}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
