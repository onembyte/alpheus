import { useCallback, useEffect, useState } from "react";
import * as api from "../api";
import type { AppSettings } from "../types";

/** Backend-persisted settings; every change saves immediately. */
export function useSettings() {
  const [settings, setLocal] = useState<AppSettings | null>(null);

  useEffect(() => {
    api.getSettings().then(setLocal).catch(() => {});
  }, []);

  const update = useCallback((patch: Partial<AppSettings>) => {
    setLocal((prev) => {
      if (!prev) return prev;
      const next = { ...prev, ...patch };
      api.setSettings(next).catch(() => {});
      return next;
    });
  }, []);

  return { settings, update };
}
