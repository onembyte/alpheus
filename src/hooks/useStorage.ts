import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "../api";
import type { Card, DiskUsage, DryRun, HistoryEntry, Tier } from "../types";

export interface ModalState {
  card: Card;
  dryRun: DryRun;
}

/**
 * All app state and actions in one place; components stay purely visual.
 * The executed card is removed optimistically — a full rescan is seconds of
 * `du`, so we only refresh usage and history after an action.
 */
export function useStorage() {
  const [usage, setUsage] = useState<DiskUsage | null>(null);
  const [cards, setCards] = useState<Card[] | null>(null);
  const [scanning, setScanning] = useState(false);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [modal, setModal] = useState<ModalState | null>(null);
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);

  const refreshUsage = useCallback(() => {
    api.diskUsage().then(setUsage).catch(() => {});
  }, []);

  const refreshHistory = useCallback(() => {
    api.getHistory().then(setHistory).catch(() => {});
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
    refreshHistory();
  }, [refreshUsage, rescan, refreshHistory]);

  const showToast = useCallback((msg: string) => {
    window.clearTimeout(toastTimer.current);
    setToast(msg);
    toastTimer.current = window.setTimeout(() => setToast(null), 6000);
  }, []);

  // Background auto-scans (Settings → Automatic scanning) push fresh cards;
  // auto-cleanups announce what they freed and land in History.
  useEffect(() => {
    const unScan = listen<Card[]>("auto-scan", (e) => {
      setCards(e.payload);
      refreshUsage();
    });
    const unClean = listen<{ freed_kb: number; count: number }>("auto-clean", (e) => {
      const gb = (e.payload.freed_kb / 1048576).toFixed(1);
      showToast(
        `Auto-cleaned ${gb} GB across ${e.payload.count} categor${e.payload.count === 1 ? "y" : "ies"}.`,
      );
      refreshHistory();
      refreshUsage();
    });
    return () => {
      unScan.then((f) => f());
      unClean.then((f) => f());
    };
  }, [refreshUsage, refreshHistory, showToast]);

  const openCard = useCallback(async (card: Card) => {
    try {
      setModal({ card, dryRun: await api.dryRun(card.id) });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const closeModal = useCallback(() => setModal(null), []);

  const confirm = useCallback(async () => {
    if (!modal) return;
    setBusy(true);
    try {
      const res = await api.execute(modal.card.id);
      showToast(res.message);
      setCards((cs) => (cs ? cs.filter((c) => c.id !== modal.card.id) : cs));
      setModal(null);
      refreshUsage();
      refreshHistory();
    } catch (e) {
      setError(String(e));
      setModal(null);
    } finally {
      setBusy(false);
    }
  }, [modal, showToast, refreshUsage, refreshHistory]);

  const totals = useMemo(() => {
    const sum = (tier: Tier) =>
      cards
        ?.filter((c) => c.tier === tier && c.action !== "explain")
        .reduce((s, c) => s + c.size_kb, 0) ?? 0;
    return { safe: sum("safe"), care: sum("with-care") };
  }, [cards]);

  return {
    usage,
    cards,
    scanning,
    history,
    modal,
    busy,
    toast,
    error,
    totals,
    rescan,
    openCard,
    closeModal,
    confirm,
    clearError: useCallback(() => setError(null), []),
  };
}
