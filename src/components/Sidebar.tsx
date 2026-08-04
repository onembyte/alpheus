import type { DiskUsage, Tier } from "../types";
import type { Theme } from "../hooks/useTheme";
import { TIER_META, TIER_ORDER } from "../tiers";
import { fmtKB } from "../format";

export type TierFilter = "all" | Tier;

interface Props {
  usage: DiskUsage | null;
  tierTotals: Record<Tier, number>;
  filter: TierFilter;
  onFilter: (f: TierFilter) => void;
  theme: Theme;
  onToggleTheme: () => void;
}

/** The Strata ring mark — open arcs, notch always facing the same corner. */
function LogoMark({ size }: { size: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      style={{ color: "var(--accent)", flex: "none" }}
    >
      <circle
        cx="50"
        cy="50"
        r="38"
        stroke="currentColor"
        strokeWidth="14"
        strokeLinecap="round"
        strokeDasharray="192.3 238.8"
        transform="rotate(-90 50 50)"
      />
      <circle
        cx="50"
        cy="50"
        r="20"
        stroke="currentColor"
        strokeWidth="12"
        strokeLinecap="round"
        strokeDasharray="83.8 125.7"
        transform="rotate(-90 50 50)"
        opacity=".6"
      />
    </svg>
  );
}

function freeStatus(usage: DiskUsage | null): { color: string; label: string; pulse: boolean } {
  if (!usage) return { color: "var(--txt3)", label: "measuring", pulse: false };
  const gb = usage.free_kb / 1048576;
  if (gb < 10) return { color: "var(--danger)", label: "critically low", pulse: true };
  if (gb < 15) return { color: "var(--warn)", label: "low", pulse: true };
  return { color: "var(--good)", label: "healthy", pulse: false };
}

export default function Sidebar(p: Props) {
  const status = freeStatus(p.usage);
  const selected = (active: boolean) =>
    active
      ? { background: "var(--sel)", boxShadow: "inset 0 1px 0 var(--edge-hi), 0 0 0 1px var(--sel-edge)" }
      : undefined;

  return (
    <div
      className="relative flex w-[230px] flex-none flex-col"
      style={{ background: "var(--side)", borderRight: "1px solid var(--hair)" }}
    >
      {/* inner left highlight, straight from the mock */}
      <div
        className="pointer-events-none absolute bottom-0 left-0 top-0 w-px"
        style={{ background: "linear-gradient(180deg, var(--edge-hi), transparent 40%)" }}
      />

      {/* traffic-light strip */}
      <div data-tauri-drag-region className="h-[52px] flex-none" />

      <div data-tauri-drag-region className="flex items-center gap-2.5 px-[18px] pb-3">
        <LogoMark size={17} />
        <span
          className="text-[11px] font-semibold uppercase"
          style={{ letterSpacing: ".11em", color: "var(--txt)" }}
        >
          Storage Manager
        </span>
        <div className="grow" />
        <span className="mono text-[9.5px]" style={{ color: "var(--txt3)" }}>
          0.1.0
        </span>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-2.5 pb-3.5">
        <div className="section-label px-2.5 pb-2 pt-3">This Mac</div>

        <div className="flex items-center gap-2.5 rounded-[11px] px-2.5 py-2">
          <svg width="18" height="18" viewBox="0 0 18 18" fill="none" style={{ flex: "none" }}>
            <rect
              x="1.6"
              y="4.4"
              width="14.8"
              height="9.2"
              rx="2.4"
              stroke="currentColor"
              strokeWidth="1.3"
              opacity=".85"
            />
            <circle cx="12.6" cy="9" r="1.5" fill="currentColor" opacity=".85" />
            <path d="M4.4 9h4.2" stroke="currentColor" strokeWidth="1.3" opacity=".5" />
          </svg>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[13px] font-medium" style={{ color: "var(--txt)" }}>
              Macintosh HD
            </div>
            <div className="mono text-[10.5px]" style={{ color: "var(--txt3)" }}>
              {p.usage
                ? `${fmtKB(p.usage.free_kb)} free of ${fmtKB(p.usage.total_kb)}`
                : "measuring…"}
            </div>
          </div>
          <div
            className={`h-[7px] w-[7px] flex-none rounded-full ${status.pulse ? "dot-pulse" : ""}`}
            style={{ background: status.color, boxShadow: `0 0 8px ${status.color}` }}
            title={status.label}
          />
        </div>

        <div className="section-label px-2.5 pb-2 pt-4">Categories</div>

        <button
          onClick={() => p.onFilter("all")}
          className="btn-focus flex items-center gap-2.5 rounded-[11px] px-2.5 py-[7px] text-left transition-colors hover:bg-(--track)"
          style={selected(p.filter === "all")}
        >
          <span className="text-[13px] font-medium" style={{ color: "var(--txt)" }}>
            All tiers
          </span>
        </button>

        {TIER_ORDER.map((tier) => {
          const meta = TIER_META[tier];
          const kb = p.tierTotals[tier];
          return (
            <button
              key={tier}
              onClick={() => p.onFilter(tier)}
              className="btn-focus flex items-center gap-2.5 rounded-[11px] px-2.5 py-[7px] text-left transition-colors hover:bg-(--track)"
              style={selected(p.filter === tier)}
            >
              <span
                className="h-2 w-2 flex-none rounded-[3px]"
                style={{ background: meta.color }}
              />
              <span className="min-w-0 flex-1 truncate text-[13px] font-medium" style={{ color: "var(--txt)" }}>
                {meta.heading}
              </span>
              <span className="mono text-[10.5px]" style={{ color: "var(--txt3)" }}>
                {kb > 0 ? fmtKB(kb) : "—"}
              </span>
            </button>
          );
        })}
      </div>

      <div className="flex flex-none gap-2 p-3" style={{ borderTop: "1px solid var(--hair)" }}>
        <button
          onClick={p.onToggleTheme}
          className="glass-chip btn-focus flex h-[30px] flex-1 items-center justify-center gap-1.5 text-[11.5px] font-medium"
          style={{ color: "var(--txt2)" }}
        >
          {p.theme === "dark" ? "Light appearance" : "Dark appearance"}
        </button>
      </div>
    </div>
  );
}
