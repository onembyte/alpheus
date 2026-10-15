import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "../api";
import { latestOnly } from "../breakdown/latest";
import { cardLinks, type Link } from "../breakdown/links";
import { initial, reduce, screen, shouldRefresh, type Crumb } from "../breakdown/state";
import type { BreakdownPayload, Card } from "../types";

/**
 * Storage detail. The walk runs in the backend at low priority; this hook
 * only folds its payloads into state (see `breakdown/state`), so nothing here
 * waits on the disk and every other view stays responsive while it measures.
 */
export function useBreakdown(cards: Card[] | null) {
  const [state, dispatch] = useReducer(reduce, initial);
  const [links, setLinks] = useState<Map<number, Link>>(() => new Map());
  const path = useRef(state.path);
  path.current = state.path;
  const latest = useRef(state);
  latest.current = state;

  const apply = useCallback((p: BreakdownPayload | null) => {
    if (p) dispatch({ type: "payload", payload: p });
  }, []);

  useEffect(() => {
    api
      .breakdownCached()
      .then((p) => dispatch({ type: "cached", payload: p }))
      .catch(() => {});
    // Several payloads a second: draw only the newest, once per frame, so a
    // busy window never works through a backlog of stale ones.
    const newest = latestOnly(apply, (flush) => requestAnimationFrame(flush));
    const un = listen<BreakdownPayload>("breakdown", (e) => newest(e.payload));
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

  // Back to the front after a while: measure again, so the window never
  // shows an old walk as if it were now. Never over a running walk.
  useEffect(() => {
    const un = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && shouldRefresh(latest.current, Date.now() / 1000)) start();
    });
    return () => {
      un.then((f) => f());
    };
  }, [start]);

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

  const view = useMemo(() => screen(state), [state]);

  // Link rows to the reclaim cards they hold. Re-located whenever there are
  // rows the last answer couldn't know: new cards, a new walk, the walk
  // ending, a row opened, a folder measured on demand.
  const session = state.live?.started ?? null;
  const running = state.live?.running ?? false;
  const depth = state.path.length;
  const busyHere = view.measuringHere;
  useEffect(() => {
    if (!cards || session === null) {
      setLinks(new Map());
      return;
    }
    const paths = [...new Set(cards.flatMap((c) => c.paths))];
    let stale = false;
    api
      .breakdownLocate(paths)
      .then((chains) => {
        if (stale) return;
        const byPath = Object.fromEntries(paths.map((p, i) => [p, chains[i] ?? []]));
        setLinks(cardLinks(cards, byPath));
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [cards, session, running, depth, busyHere]);

  const openFullDiskAccess = useCallback(() => {
    api.openFullDiskAccess().catch(() => {});
  }, []);

  return { screen: view, links, start, cancel, open, up, openFullDiskAccess };
}
