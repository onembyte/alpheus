import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as api from "./api";
import type { Card, DiskUsage, DryRun, HistoryEntry, Tier } from "./types";
import DiskHeader from "./components/DiskHeader";
import CardItem from "./components/CardItem";
import ConfirmModal from "./components/ConfirmModal";
import HistoryList from "./components/HistoryList";

const SECTIONS: { tier: Tier; heading: string; sub: string }[] = [
  {
    tier: "safe",
    heading: "Safe to reclaim",
    sub: "Regenerable caches and build artifacts — everything comes back on demand.",
  },
  {
    tier: "with-care",
    heading: "Needs a decision",
    sub: "Removable, but read what dies first. The confirm dialog lists it exactly.",
  },
  {
    tier: "manual",
    heading: "Manual / informational",
    sub: "This app won't touch these — it just explains where the space is.",
  },
];

export default function App() {
  const [usage, setUsage] = useState<DiskUsage | null>(null);
  const [cards, setCards] = useState<Card[] | null>(null);
  const [scanning, setScanning] = useState(false);
  const [tab, setTab] = useState<"cards" | "history">("cards");
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [modal, setModal] = useState<{ card: Card; dryRun: DryRun } | null>(null);
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);

  const refreshUsage = useCallback(() => {
    api.diskUsage().then(setUsage).catch(() => {});
  }, []);

  const rescan = useCallback(() => {
    setScanning(true);
    setError(null);
    api
      .scan()
      .then(setCards)
      .catch((e) => setError(String(e)))
      .finally(() => {
        setScanning(false);
        refreshUsage();
      });
  }, [refreshUsage]);

  useEffect(() => {
    refreshUsage();
    rescan();
    api.getHistory().then(setHistory).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const showToast = (msg: string) => {
    window.clearTimeout(toastTimer.current);
    setToast(msg);
    toastTimer.current = window.setTimeout(() => setToast(null), 6000);
  };

  const openCard = async (card: Card) => {
    try {
      setModal({ card, dryRun: await api.dryRun(card.id) });
    } catch (e) {
      setError(String(e));
    }
  };

  const confirm = async () => {
    if (!modal) return;
    setBusy(true);
    try {
      const res = await api.execute(modal.card.id);
      showToast(res.message);
      setCards((cs) => (cs ? cs.filter((c) => c.id !== modal.card.id) : cs));
      setModal(null);
      refreshUsage();
      api.getHistory().then(setHistory).catch(() => {});
    } catch (e) {
      setError(String(e));
      setModal(null);
    } finally {
      setBusy(false);
    }
  };

  const totals = useMemo(() => {
    const sum = (tier: Tier) =>
      cards
        ?.filter((c) => c.tier === tier && c.action !== "explain")
        .reduce((s, c) => s + c.size_kb, 0) ?? 0;
    return { safe: sum("safe"), care: sum("with-care") };
  }, [cards]);

  return (
    <div className="flex h-screen flex-col">
      <DiskHeader
        usage={usage}
        safeKb={totals.safe}
        careKb={totals.care}
        scanning={scanning}
        historyCount={history.length}
        tab={tab}
        onTab={setTab}
        onRescan={rescan}
      />

      <main className="min-h-0 grow overflow-y-auto px-5 py-4">
        {error && (
          <div className="mb-3 flex items-start gap-3 rounded-lg border border-red-800/60 bg-red-900/20 px-3 py-2 text-[12px] text-red-200">
            <span className="grow">{error}</span>
            <button onClick={() => setError(null)} className="shrink-0 font-semibold">
              ✕
            </button>
          </div>
        )}

        {tab === "history" ? (
          <HistoryList entries={history} />
        ) : scanning && !cards ? (
          <div className="py-16 text-center text-sm text-neutral-500">
            Scanning ~/Documents, Xcode, caches, colima…
            <div className="mt-1 text-[12px] text-neutral-600">
              the first run can take up to a minute
            </div>
          </div>
        ) : cards && cards.length === 0 ? (
          <div className="py-16 text-center text-sm text-neutral-500">
            Nothing left to reclaim — the disk is clean.
            <div className="mt-1 text-[12px] text-neutral-600">
              Rescan anytime; caches grow back on their own.
            </div>
          </div>
        ) : (
          SECTIONS.map(({ tier, heading, sub }) => {
            const group = cards?.filter((c) => c.tier === tier) ?? [];
            if (group.length === 0) return null;
            const groupTotal = group
              .filter((c) => c.action !== "explain")
              .reduce((s, c) => s + c.size_kb, 0);
            return (
              <section key={tier} className="mb-6">
                <div className="mb-2 flex items-baseline gap-2">
                  <h2 className="text-[13px] font-semibold uppercase tracking-wide text-neutral-300">
                    {heading}
                  </h2>
                  {groupTotal > 0 && (
                    <span className="text-[12px] tabular-nums text-neutral-500">
                      {(groupTotal / 1048576).toFixed(1)} GB
                    </span>
                  )}
                  <span className="text-[12px] text-neutral-600">— {sub}</span>
                </div>
                <div className="space-y-2">
                  {group.map((card) => (
                    <CardItem key={card.id} card={card} onAction={openCard} />
                  ))}
                </div>
              </section>
            );
          })
        )}
      </main>

      {toast && (
        <div className="pointer-events-none fixed inset-x-0 bottom-5 z-40 flex justify-center">
          <div className="rounded-full border border-neutral-700 bg-neutral-800 px-4 py-2 text-[13px] text-neutral-100 shadow-xl">
            {toast}
          </div>
        </div>
      )}

      {modal && (
        <ConfirmModal
          key={modal.card.id}
          card={modal.card}
          dryRun={modal.dryRun}
          busy={busy}
          onConfirm={confirm}
          onCancel={() => setModal(null)}
        />
      )}
    </div>
  );
}
