import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "../api";
import { initial, reduce, screen, type Crumb } from "../breakdown/state";
import type { BreakdownPayload } from "../types";

/**
 * Storage detail. The walk runs in the backend at low priority; this hook
 * only folds its payloads into state (see `breakdown/state`), so nothing here
 * waits on the disk and every other view stays responsive while it measures.
 */
export function useBreakdown() {
  const [state, dispatch] = useReducer(reduce, initial);
  const path = useRef(state.path);
  path.current = state.path;

  const apply = useCallback((p: BreakdownPayload | null) => {
    if (p) dispatch({ type: "payload", payload: p });
  }, []);

  useEffect(() => {
    api
      .breakdownCached()
      .then((p) => dispatch({ type: "cached", payload: p }))
      .catch(() => {});
    const un = listen<BreakdownPayload>("breakdown", (e) => apply(e.payload));
    // The app started a walk at launch; this picks it up at once (or starts
    // one if it already finished) instead of waiting for the next event.
    api.breakdownStart().then(apply).catch(() => {});
    return () => {
      un.then((f) => f());
    };
  }, [apply]);

  /** Starts a walk, or picks up the one already running. */
  const start = useCallback(() => {
    api.breakdownStart().then(apply).catch(() => {});
  }, [apply]);

  const cancel = useCallback(() => {
    api.breakdownCancel().catch(() => {});
  }, []);

  /** Opens a row; its children follow at once and then with every update. */
  const open = useCallback(
    (crumb: Crumb) => {
      dispatch({ type: "open", crumb });
      api.breakdownFocus(crumb.id).then(apply).catch(() => {});
    },
    [apply],
  );

  /** Back to the breadcrumb at `to`; -1 is the top. */
  const up = useCallback(
    (to: number) => {
      const id = to < 0 ? null : (path.current[to]?.id ?? null);
      dispatch({ type: "up", to });
      api.breakdownFocus(id).then(apply).catch(() => {});
    },
    [apply],
  );

  return { screen: useMemo(() => screen(state), [state]), start, cancel, open, up };
}
