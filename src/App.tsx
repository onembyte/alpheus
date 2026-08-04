import { useState } from "react";
import { useStorage } from "./hooks/useStorage";
import { TIER_META, TIER_ORDER } from "./tiers";
import DiskHeader from "./components/DiskHeader";
import CardItem from "./components/CardItem";
import ConfirmModal from "./components/ConfirmModal";
import HistoryList from "./components/HistoryList";

export default function App() {
  const s = useStorage();
  const [tab, setTab] = useState<"cards" | "history">("cards");

  return (
    <div className="flex h-screen flex-col">
      <DiskHeader
        usage={s.usage}
        safeKb={s.totals.safe}
        careKb={s.totals.care}
        scanning={s.scanning}
        historyCount={s.history.length}
        tab={tab}
        onTab={setTab}
        onRescan={s.rescan}
      />

      <main className="min-h-0 grow overflow-y-auto px-5 py-4">
        {s.error && (
          <div className="mb-3 flex items-start gap-3 rounded-lg border border-red-800/60 bg-red-900/20 px-3 py-2 text-[12px] text-red-200">
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

        {tab === "history" ? (
          <HistoryList entries={s.history} />
        ) : s.scanning && !s.cards ? (
          <div className="py-16 text-center text-sm text-neutral-500">
            Scanning ~/Documents, Xcode, caches, colima…
            <div className="mt-1 text-[12px] text-neutral-600">
              the first run can take up to a minute
            </div>
          </div>
        ) : s.cards && s.cards.length === 0 ? (
          <div className="py-16 text-center text-sm text-neutral-500">
            Nothing left to reclaim — the disk is clean.
            <div className="mt-1 text-[12px] text-neutral-600">
              Rescan anytime; caches grow back on their own.
            </div>
          </div>
        ) : (
          <div className={s.scanning ? "pointer-events-none opacity-60" : undefined}>
            {TIER_ORDER.map((tier) => {
              const meta = TIER_META[tier];
              const group = s.cards?.filter((c) => c.tier === tier) ?? [];
              if (group.length === 0) return null;
              const groupTotal = group
                .filter((c) => c.action !== "explain")
                .reduce((sum, c) => sum + c.size_kb, 0);
              return (
                <section key={tier} className="mb-6">
                  <div className="mb-2 flex items-baseline gap-2">
                    <h2 className="text-[13px] font-semibold uppercase tracking-wide text-neutral-300">
                      {meta.heading}
                    </h2>
                    {groupTotal > 0 && (
                      <span className="text-[12px] tabular-nums text-neutral-500">
                        {(groupTotal / 1048576).toFixed(1)} GB
                      </span>
                    )}
                    <span className="text-[12px] text-neutral-600">— {meta.sub}</span>
                  </div>
                  <div className="space-y-2">
                    {group.map((card) => (
                      <CardItem key={card.id} card={card} onAction={s.openCard} />
                    ))}
                  </div>
                </section>
              );
            })}
          </div>
        )}
      </main>

      {s.toast && (
        <div className="pointer-events-none fixed inset-x-0 bottom-5 z-40 flex justify-center">
          <div className="rounded-full border border-neutral-700 bg-neutral-800 px-4 py-2 text-[13px] text-neutral-100 shadow-xl">
            {s.toast}
          </div>
        </div>
      )}

      {s.modal && (
        <ConfirmModal
          key={s.modal.card.id}
          card={s.modal.card}
          dryRun={s.modal.dryRun}
          busy={s.busy}
          onConfirm={s.confirm}
          onCancel={s.closeModal}
        />
      )}
    </div>
  );
}
