import { useEffect, useMemo, useState } from "react";
import { useStorage } from "./hooks/useStorage";
import { useSettings } from "./hooks/useSettings";
import { useTheme } from "./hooks/useTheme";
import { cardColor, TIER_META, TIER_ORDER } from "./tiers";
import type { Card, Tier } from "./types";
import Wallpaper from "./components/Wallpaper";
import Sidebar, { type TierFilter } from "./components/Sidebar";
import TopBar, { type View } from "./components/TopBar";
import OverviewPanel from "./components/OverviewPanel";
import CardRow from "./components/CardRow";
import Sheet from "./components/Sheet";
import HistoryPanel from "./components/HistoryPanel";
import SettingsPanel from "./components/SettingsPanel";
import { cardMatches, isEmptyQuery, parseQuery } from "./search";
import { fmtKB } from "./format";

function ScanningState() {
  return (
    <div className="fade-up flex flex-col items-center gap-5 py-16">
      <div className="relative h-[170px] w-[170px]">
        <svg
          className="spin-fast absolute inset-0"
          width="170"
          height="170"
          viewBox="0 0 140 140"
          style={{ animationDuration: "1.4s" }}
        >
          <circle cx="70" cy="70" r="54" fill="none" stroke="var(--track)" strokeWidth="9" />
          <path
            d="M70 16a54 54 0 0 1 46.8 27"
            fill="none"
            stroke="var(--accent)"
            strokeWidth="9"
            strokeLinecap="round"
            style={{ filter: "drop-shadow(0 0 10px var(--accent))" }}
          />
        </svg>
        <div
          className="spin-slow absolute rounded-full"
          style={{ inset: -12, border: "1px dashed var(--edge-lo)" }}
        />
      </div>
      <div className="text-center">
        <div className="text-[15px] font-semibold" style={{ color: "var(--txt)" }}>
          Scanning this Mac
        </div>
        <div className="mono mt-1.5 text-[11px]" style={{ color: "var(--txt2)" }}>
          ~/Documents · Xcode · caches · colima · backups — first run can take a minute
        </div>
      </div>
    </div>
  );
}

export default function App() {
  const s = useStorage();
  const { settings, update: updateSettings } = useSettings();
  const { mode, setMode } = useTheme();
  const [view, setView] = useState<View>("reclaim");
  const [filter, setFilter] = useState<TierFilter>("all");
  const [query, setQuery] = useState("");
  const parsedQuery = useMemo(() => parseQuery(query), [query]);
  const searching = !isEmptyQuery(parsedQuery);

  const toggleAuto = (id: string) => {
    if (!settings) return;
    const ids = settings.auto_clean_ids.includes(id)
      ? settings.auto_clean_ids.filter((x) => x !== id)
      : [...settings.auto_clean_ids, id];
    updateSettings({ auto_clean_ids: ids });
  };

  // Cmd+, opens Settings, like any other macOS app.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === ",") {
        e.preventDefault();
        setView("settings");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  /** Stable card→color assignment by size order within the current scan. */
  const colorOf = useMemo(() => {
    const map = new Map<string, string>();
    (s.cards ?? [])
      .filter((c) => c.action !== "explain")
      .forEach((c, i) => map.set(c.id, cardColor(i)));
    return (c: Card) => map.get(c.id) ?? "var(--c7)";
  }, [s.cards]);

  const tierTotals = useMemo(() => {
    const totals = { safe: 0, "with-care": 0, manual: 0 } as Record<Tier, number>;
    for (const c of s.cards ?? []) totals[c.tier] += c.size_kb;
    return totals;
  }, [s.cards]);

  const maxKb = useMemo(
    () => Math.max(0, ...(s.cards ?? []).map((c) => c.size_kb)),
    [s.cards],
  );

  const visibleTiers = TIER_ORDER.filter((t) => filter === "all" || filter === t);

  return (
    <div className="relative h-screen overflow-hidden" style={{ background: "var(--w1)" }}>
      <Wallpaper />

      <div
        className="relative z-10 flex h-full"
        style={{
          background: "var(--win)",
          backdropFilter: "blur(var(--blur)) saturate(var(--sat))",
          WebkitBackdropFilter: "blur(var(--blur)) saturate(var(--sat))",
        }}
      >
        <Sidebar
          usage={s.usage}
          tierTotals={tierTotals}
          warnBelowGb={settings?.notify_below_gb ?? 15}
          filter={filter}
          onFilter={(f) => {
            setFilter(f);
            setView("reclaim");
          }}
          volumeActive={view === "reclaim"}
          onSelectVolume={() => {
            setFilter("all");
            setView("reclaim");
          }}
          settingsActive={view === "settings"}
          onOpenSettings={() => setView("settings")}
        />

        <div className="flex min-w-0 flex-1 flex-col">
          <TopBar
            view={view}
            onView={setView}
            scanning={s.scanning}
            onRescan={s.rescan}
            reclaimableKb={tierTotals.safe + tierTotals["with-care"]}
            historyCount={s.history.length}
            query={query}
            onQuery={setQuery}
          />

          <main className="min-h-0 grow overflow-y-auto p-5">
            {s.error && (
              <div
                className="mb-4 flex items-start gap-3 rounded-[13px] px-3.5 py-2.5 text-[12px]"
                style={{
                  color: "var(--txt)",
                  background: "color-mix(in srgb, var(--danger) 14%, transparent)",
                  boxShadow: "0 0 0 1px color-mix(in srgb, var(--danger) 30%, transparent)",
                }}
              >
                <span className="grow">{s.error}</span>
                <button
                  onClick={s.clearError}
                  aria-label="Dismiss error"
                  className="btn-focus shrink-0 font-semibold"
                >
                  ✕
                </button>
              </div>
            )}

            {view === "settings" ? (
              <SettingsPanel
                mode={mode}
                onMode={setMode}
                settings={settings}
                update={updateSettings}
              />
            ) : view === "history" ? (
              <HistoryPanel entries={s.history} />
            ) : s.scanning && !s.cards ? (
              <ScanningState />
            ) : s.cards && s.cards.length === 0 ? (
              <div className="glass-card fade-up px-[18px] py-14 text-center">
                <div className="text-[13px]" style={{ color: "var(--txt2)" }}>
                  Nothing left to reclaim — the disk is clean.
                </div>
                <div className="mono mt-1 text-[10.5px]" style={{ color: "var(--txt3)" }}>
                  rescan anytime · caches grow back on their own
                </div>
              </div>
            ) : (
              <div
                className={`flex flex-col gap-4 ${s.scanning ? "pointer-events-none opacity-60" : ""}`}
              >
                {s.usage && s.cards && filter === "all" && !searching && (
                  <OverviewPanel
                    usage={s.usage}
                    cards={s.cards}
                    warnBelowGb={settings?.notify_below_gb ?? 15}
                    onFreeUp={() => setFilter("safe")}
                  />
                )}

                {(s.cards ?? []).filter(
                  (c) =>
                    (filter === "all" || c.tier === filter) && cardMatches(c, parsedQuery),
                ).length === 0 && (
                  <div className="glass-card fade-up px-[18px] py-12 text-center">
                    <div className="text-[13px]" style={{ color: "var(--txt2)" }}>
                      {searching ? "Nothing matches that filter." : "Nothing in this tier right now."}
                    </div>
                    <div className="mono mt-1 text-[10.5px]" style={{ color: "var(--txt3)" }}>
                      {searching
                        ? "try size:>500MB, tier:safe, or plain text — Esc clears"
                        : "rescan or pick another category on the left"}
                    </div>
                  </div>
                )}

                {visibleTiers.map((tier) => {
                  const meta = TIER_META[tier];
                  const group = (s.cards ?? []).filter(
                    (c) => c.tier === tier && cardMatches(c, parsedQuery),
                  );
                  if (group.length === 0) return null;
                  const groupTotal = group
                    .filter((c) => c.action !== "explain")
                    .reduce((sum, c) => sum + c.size_kb, 0);
                  return (
                    <section key={tier} className="glass-card fade-up px-[18px] pb-1 pt-4">
                      <div className="mb-1.5 flex items-baseline gap-2.5">
                        <span
                          className="h-2 w-2 flex-none self-center rounded-[3px]"
                          style={{ background: meta.color }}
                        />
                        <h2 className="section-label">{meta.heading}</h2>
                        {groupTotal > 0 && (
                          <span className="mono text-[10.5px]" style={{ color: "var(--txt2)" }}>
                            {fmtKB(groupTotal)}
                          </span>
                        )}
                        <span
                          className="hidden min-w-0 truncate text-[11px] md:block"
                          style={{ color: "var(--txt3)" }}
                        >
                          {meta.sub}
                        </span>
                      </div>
                      {group.map((card) => (
                        <CardRow
                          key={card.id}
                          card={card}
                          color={colorOf(card)}
                          maxKb={maxKb}
                          onAction={s.openCard}
                          autoOn={
                            card.tier === "safe" && card.action === "delete" && settings
                              ? settings.auto_clean_ids.includes(card.id)
                              : null
                          }
                          onToggleAuto={toggleAuto}
                        />
                      ))}
                    </section>
                  );
                })}
              </div>
            )}
          </main>
        </div>
      </div>

      {s.toast && (
        <div className="pointer-events-none absolute inset-x-0 bottom-6 z-40 flex justify-center">
          <div
            className="fade-up flex items-center gap-2.5 rounded-[14px] px-[18px] py-[11px]"
            style={{
              background: "linear-gradient(160deg, var(--glass-a), var(--glass-b)), var(--win)",
              backdropFilter: "blur(40px) saturate(200%)",
              WebkitBackdropFilter: "blur(40px) saturate(200%)",
              boxShadow:
                "0 20px 50px -14px rgba(0,0,0,.6), inset 0 1px 0 var(--edge-hi), 0 0 0 1px var(--edge)",
            }}
          >
            <span
              className="flex h-[18px] w-[18px] flex-none items-center justify-center rounded-full"
              style={{ background: "var(--good)" }}
            >
              <svg width="11" height="11" viewBox="0 0 10 10" fill="none">
                <path
                  d="M1.6 5.2 3.9 7.5 8.4 2.6"
                  stroke="var(--on-good)"
                  strokeWidth="1.9"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </span>
            <span className="text-[12px] font-medium" style={{ color: "var(--txt)" }}>
              {s.toast}
            </span>
          </div>
        </div>
      )}

      {s.modal && (
        <Sheet
          key={s.modal.card.id}
          card={s.modal.card}
          dryRun={s.modal.dryRun}
          freeKb={s.usage?.free_kb ?? 0}
          busy={s.busy}
          onConfirm={s.confirm}
          onCancel={s.closeModal}
        />
      )}
    </div>
  );
}
