import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import * as drive from "../drive/api";
import type { DriveFile, DriveQuota } from "../drive/api";

export interface DupeSet {
  key: string;
  name: string;
  eachBytes: number;
  copies: number;
  /** Bytes freed by trashing everything except the kept copy. */
  reclaimBytes: number;
  /** Most recently modified copy — the one we keep. */
  keep: DriveFile;
  redundant: DriveFile[];
}

function groupDupes(files: DriveFile[]): DupeSet[] {
  const byHash = new Map<string, DriveFile[]>();
  for (const f of files) {
    if (!f.md5 || f.sizeBytes <= 0) continue;
    const key = `${f.md5}:${f.sizeBytes}`;
    byHash.get(key)?.push(f) ?? byHash.set(key, [f]);
  }
  const sets: DupeSet[] = [];
  for (const [key, group] of byHash) {
    if (group.length < 2) continue;
    const sorted = [...group].sort(
      (a, b) => Date.parse(b.modifiedTime) - Date.parse(a.modifiedTime),
    );
    const [keep, ...redundant] = sorted;
    sets.push({
      key,
      name: keep.name,
      eachBytes: keep.sizeBytes,
      copies: group.length,
      reclaimBytes: redundant.reduce((s, f) => s + f.sizeBytes, 0),
      keep,
      redundant,
    });
  }
  return sets.sort((a, b) => b.reclaimBytes - a.reclaimBytes);
}

export function useDrive() {
  const [connected, setConnected] = useState<boolean | null>(null);
  const [quota, setQuota] = useState<DriveQuota | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [progress, setProgress] = useState(0);
  const [dupes, setDupes] = useState<DupeSet[] | null>(null);
  const [largest, setLargest] = useState<DriveFile[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshStatus = useCallback(() => {
    invoke<boolean>("google_status").then(setConnected).catch(() => setConnected(false));
  }, []);

  useEffect(refreshStatus, [refreshStatus]);

  const refreshQuota = useCallback(async () => {
    try {
      setQuota(await drive.fetchQuota());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const analyze = useCallback(async () => {
    setAnalyzing(true);
    setError(null);
    setProgress(0);
    try {
      setQuota(await drive.fetchQuota());
      const files = await drive.listOwnFiles(setProgress);
      setDupes(groupDupes(files));
      setLargest(
        [...files].sort((a, b) => b.sizeBytes - a.sizeBytes).slice(0, 12),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setAnalyzing(false);
    }
  }, []);

  const connect = useCallback(
    async (clientId: string, clientSecret: string) => {
      setBusy(true);
      setError(null);
      try {
        await invoke("google_connect", { clientId, clientSecret });
        setConnected(true);
        return true;
      } catch (e) {
        setError(String(e));
        return false;
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const disconnect = useCallback(async () => {
    setBusy(true);
    try {
      await invoke("google_disconnect");
    } finally {
      setBusy(false);
      setConnected(false);
      setQuota(null);
      setDupes(null);
      setLargest(null);
    }
  }, []);

  /** Trash every redundant copy of a set; the newest copy stays. */
  const trashDuplicates = useCallback(
    async (set: DupeSet) => {
      setBusy(true);
      setError(null);
      try {
        for (const f of set.redundant) await drive.trashFile(f.id);
        setDupes((d) => d?.filter((s) => s.key !== set.key) ?? null);
        await refreshQuota();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [refreshQuota],
  );

  const trashSingle = useCallback(
    async (file: DriveFile) => {
      setBusy(true);
      setError(null);
      try {
        await drive.trashFile(file.id);
        setLargest((l) => l?.filter((f) => f.id !== file.id) ?? null);
        await refreshQuota();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [refreshQuota],
  );

  const emptyDriveTrash = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await drive.emptyTrash();
      await refreshQuota();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [refreshQuota]);

  return {
    connected,
    refreshStatus,
    quota,
    refreshQuota,
    analyzing,
    progress,
    analyze,
    dupes,
    largest,
    busy,
    error,
    clearError: useCallback(() => setError(null), []),
    connect,
    disconnect,
    trashDuplicates,
    trashSingle,
    emptyDriveTrash,
  };
}
