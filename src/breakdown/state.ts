import type { BreakdownNode, BreakdownPayload, NodeState } from "../types";

/**
 * Storage detail state, kept pure so it can be tested without a webview.
 *
 * The backend walks the disk and sends payloads; this folds them in. The
 * cached result of the last finished walk is shown until the live walk
 * reports, then lends each live row its "last time" size, so a row still
 * being measured can show what it held before.
 */

export interface Crumb {
  id: number;
  name: string;
}

export interface State {
  live: BreakdownPayload | null;
  cached: BreakdownPayload | null;
  /** Rows opened below the root, outermost first. Ids are the live walk's. */
  path: Crumb[];
  /** The opened row with its children, once the backend has sent it. */
  focus: BreakdownNode | null;
}

export type Action =
  | { type: "cached"; payload: BreakdownPayload | null }
  | { type: "payload"; payload: BreakdownPayload }
  | { type: "open"; crumb: Crumb }
  /** Back to `path[to]`; -1 is the root. */
  | { type: "up"; to: number };

export const initial: State = { live: null, cached: null, path: [], focus: null };

export function reduce(s: State, a: Action): State {
  switch (a.type) {
    case "cached":
      return { ...s, cached: a.payload };
    case "open":
      return { ...s, path: [...s.path, a.crumb], focus: null };
    case "up":
      return { ...s, path: s.path.slice(0, a.to + 1), focus: null };
    case "payload": {
      const p = a.payload;
      let path = s.path;
      let focus = s.focus;
      if (s.live && p.started !== s.live.started) {
        // A new walk renumbers every node. The category the user is in keeps
        // its name, so stay there; anything deeper isn't known yet.
        const same = path[0] && p.root.children.find((c) => c.name === path[0].name);
        path = same ? [{ id: same.id, name: same.name }] : [];
        focus = null;
      }
      const want = path.length ? path[path.length - 1].id : null;
      // Payloads sent before the user moved still carry the old focus.
      if (p.focus && p.focus.id === want) focus = p.focus;
      else if (focus?.id !== want) focus = null;
      return { ...s, live: p, path, focus };
    }
  }
}

export interface Row {
  id: number;
  name: string;
  bytes: number;
  /** What the same row held in the last finished walk. */
  previous: number | null;
  state: NodeState;
  /** Fraction of the parent, 0 to 1. */
  share: number;
  /** Clicking it shows what is inside. */
  openable: boolean;
}

export interface Screen {
  title: string | null;
  crumbs: Crumb[];
  total: number;
  rows: Row[];
  other: { count: number; bytes: number } | null;
  running: boolean;
  /** Showing the cached walk because no live one has reported yet. */
  preview: boolean;
  /** When the walk shown finished (Unix seconds); null while measuring. */
  asOf: number | null;
  /** A row was opened and its children haven't arrived yet. */
  loading: boolean;
  /** Bytes sitting directly in the opened folder, beside its subfolders. */
  filesHere: number | null;
  /** The opened row is itself still being measured (its insides, on demand). */
  measuringHere: boolean;
  files: number;
  unreadable: number;
}

export function screen(s: State): Screen {
  const source = s.live ?? s.cached;
  const preview = !s.live && !!s.cached;
  const base = {
    crumbs: s.path,
    running: !!s.live?.running,
    preview,
    asOf: source?.finished ?? null,
    files: source?.files ?? 0,
    unreadable: source?.unreadable ?? 0,
  };
  const empty = { total: 0, rows: [], other: null, filesHere: null, measuringHere: false };
  if (!source) return { ...base, ...empty, title: null, loading: false };

  const parent = s.path.length === 0 ? source.root : s.focus;
  if (!parent) {
    return { ...base, ...empty, title: s.path[s.path.length - 1].name, loading: true };
  }
  const before = preview ? null : previousSizes(s.cached, s.path);
  return {
    ...base,
    title: parent.name,
    total: parent.bytes,
    rows: parent.children.map((c) => ({
      id: c.id,
      name: c.name,
      bytes: c.bytes,
      previous: before?.get(c.name) ?? null,
      state: c.state,
      share: parent.bytes > 0 ? c.bytes / parent.bytes : 0,
      openable: c.openable,
    })),
    other: parent.other_count > 0 ? { count: parent.other_count, bytes: parent.other_bytes } : null,
    loading: false,
    // At the top the categories add up to the disk; files only sit in folders.
    filesHere: s.path.length > 0 && parent.own_bytes > 0 ? parent.own_bytes : null,
    measuringHere: s.path.length > 0 && parent.state !== "done",
  };
}

/** Sizes, by name, of the cached rows at the same place in the tree. */
function previousSizes(cached: BreakdownPayload | null, path: Crumb[]): Map<string, number> | null {
  let node: BreakdownNode | undefined = cached?.root;
  for (const crumb of path) node = node?.children.find((c) => c.name === crumb.name);
  return node ? new Map(node.children.map((c) => [c.name, c.bytes])) : null;
}
