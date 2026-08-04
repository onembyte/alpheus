import { fmtKB } from "../format";

export type View = "reclaim" | "history" | "settings";

interface Props {
  view: View;
  onView: (v: View) => void;
  scanning: boolean;
  onRescan: () => void;
  reclaimableKb: number;
  historyCount: number;
}

const TABS: { key: View; label: string }[] = [
  { key: "reclaim", label: "Reclaim" },
  { key: "history", label: "History" },
];

export default function TopBar(p: Props) {
  return (
    <div
      data-tauri-drag-region
      className="flex h-[58px] flex-none items-center gap-3.5 px-[18px]"
      style={{ borderBottom: "1px solid var(--hair)" }}
    >
      <div data-tauri-drag-region className="min-w-[178px]">
        <div data-tauri-drag-region className="text-[14px] font-semibold" style={{ color: "var(--txt)" }}>
          {p.view === "reclaim" ? "Reclaim" : p.view === "history" ? "History" : "Settings"}
        </div>
        <div data-tauri-drag-region className="mono text-[10.5px]" style={{ color: "var(--txt3)" }}>
          {p.view === "reclaim"
            ? `${fmtKB(p.reclaimableKb)} reclaimable · du actuals`
            : p.view === "history"
              ? `${p.historyCount} action${p.historyCount === 1 ? "" : "s"} logged`
              : "appearance · about"}
        </div>
      </div>

      {/* segmented control */}
      <div
        className="flex gap-[3px] rounded-[11px] p-[3px]"
        style={{ background: "var(--track)", boxShadow: "inset 0 1px 2px rgba(0,0,0,.14)" }}
      >
        {TABS.map((t) => {
          const active = p.view === t.key;
          return (
            <button
              key={t.key}
              onClick={() => p.onView(t.key)}
              className="btn-focus rounded-lg px-3.5 py-1.5 text-[12px] font-medium transition-all"
              style={{
                color: active ? "var(--txt)" : "var(--txt3)",
                background: active ? "var(--glass-a)" : "transparent",
                boxShadow: active
                  ? "inset 0 1px 0 var(--edge-hi), 0 0 0 1px var(--edge-lo)"
                  : "none",
              }}
            >
              {t.label}
            </button>
          );
        })}
      </div>

      <div data-tauri-drag-region className="grow" />

      <button
        onClick={p.onRescan}
        disabled={p.scanning}
        className="btn-focus relative flex h-[30px] items-center gap-[7px] overflow-hidden rounded-[9px] px-[15px] text-[12px] font-semibold disabled:opacity-70"
        style={{
          background:
            "linear-gradient(180deg, color-mix(in srgb, var(--accent) 55%, transparent), color-mix(in srgb, var(--accent) 28%, transparent))",
          boxShadow:
            "inset 0 1px 0 rgba(255,255,255,.6), 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent), 0 6px 18px -6px color-mix(in srgb, var(--accent) 60%, transparent)",
          color: "#fff",
          textShadow: "0 1px 2px rgba(0,0,0,.25)",
        }}
      >
        {!p.scanning && <div className="sheen" />}
        {p.scanning ? (
          <svg className="spin-fast" width="13" height="13" viewBox="0 0 14 14" fill="none">
            <circle cx="7" cy="7" r="5.4" stroke="rgba(255,255,255,.35)" strokeWidth="1.8" />
            <path
              d="M12.4 7A5.4 5.4 0 0 0 7 1.6"
              stroke="#fff"
              strokeWidth="1.8"
              strokeLinecap="round"
            />
          </svg>
        ) : (
          <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
            <path
              d="M7 1.6v3M7 9.4v3M1.6 7h3M9.4 7h3M3.2 3.2l2.1 2.1M8.7 8.7l2.1 2.1M10.8 3.2 8.7 5.3M5.3 8.7 3.2 10.8"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </svg>
        )}
        <span className="relative">{p.scanning ? "Scanning…" : "Rescan"}</span>
      </button>
    </div>
  );
}
