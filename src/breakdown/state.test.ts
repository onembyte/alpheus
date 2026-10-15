import { describe, expect, it } from "vitest";
import { initial, reduce, screen, type Action, type State } from "./state";
import type { BreakdownNode, BreakdownPayload } from "../types";

let nextId = 1;
function node(
  name: string,
  bytes: number,
  children: BreakdownNode[] = [],
  extra: Partial<BreakdownNode> = {},
): BreakdownNode {
  return {
    id: nextId++,
    name,
    bytes,
    own_bytes: 0,
    state: "done",
    children,
    other_count: 0,
    other_bytes: 0,
    ...extra,
  };
}

function payload(root: BreakdownNode, extra: Partial<BreakdownPayload> = {}): BreakdownPayload {
  return {
    root,
    focus: null,
    files: 10,
    unreadable: 0,
    running: true,
    started: 100,
    finished: null,
    ...extra,
  };
}

const run = (...actions: Action[]): State => actions.reduce(reduce, initial);

describe("preview", () => {
  it("shows the cached walk until a live one reports", () => {
    const cached = payload(node("Mac", 300, [node("Developer", 200), node("Documents", 100)]), {
      running: false,
      finished: 90,
    });
    const s = screen(run({ type: "cached", payload: cached }));
    expect(s.preview).toBe(true);
    expect(s.asOf).toBe(90);
    expect(s.rows.map((r) => r.name)).toEqual(["Developer", "Documents"]);
  });

  it("shows nothing, not an error, with no cache and no walk", () => {
    const s = screen(run({ type: "cached", payload: null }));
    expect(s.rows).toEqual([]);
    expect(s.preview).toBe(false);
  });
});

describe("live walk", () => {
  it("replaces the preview and keeps last time's size beside each row", () => {
    const cached = payload(node("Mac", 300, [node("Developer", 200), node("Documents", 100)]), {
      running: false,
      finished: 90,
    });
    const live = payload(
      node("Mac", 300, [node("Developer", 50, [], { state: "measuring" }), node("Mail", 5)]),
    );
    const s = screen(run({ type: "cached", payload: cached }, { type: "payload", payload: live }));
    expect(s.preview).toBe(false);
    expect(s.running).toBe(true);
    const dev = s.rows.find((r) => r.name === "Developer")!;
    expect(dev.previous).toBe(200);
    expect(dev.state).toBe("measuring");
    expect(s.rows.find((r) => r.name === "Mail")!.previous).toBeNull();
  });

  it("gives each row its share of the parent, and an 'other' row past the cap", () => {
    const live = payload(
      node("Mac", 400, [node("A", 300), node("B", 60)], { other_count: 4, other_bytes: 40 }),
    );
    const s = screen(run({ type: "payload", payload: live }));
    expect(s.total).toBe(400);
    expect(s.rows[0].share).toBeCloseTo(0.75);
    expect(s.other).toEqual({ count: 4, bytes: 40 });
  });
});

describe("drilling in", () => {
  const dev = node("Developer", 200);
  const live = payload(node("Mac", 300, [dev, node("Documents", 100)]));

  it("waits for the opened row's children, then shows them", () => {
    let s = run({ type: "payload", payload: live }, { type: "open", crumb: { id: dev.id, name: "Developer" } });
    expect(screen(s).loading).toBe(true);
    expect(screen(s).crumbs.map((c) => c.name)).toEqual(["Developer"]);

    const focus = node("Developer", 200, [node("dev", 150), node(".cargo", 50)], { id: dev.id });
    s = reduce(s, { type: "payload", payload: { ...live, focus } });
    const sc = screen(s);
    expect(sc.loading).toBe(false);
    expect(sc.title).toBe("Developer");
    expect(sc.rows.map((r) => r.name)).toEqual(["dev", ".cargo"]);
  });

  it("ignores a late payload that still carries the row the user left", () => {
    const docs = live.root.children[1];
    const devFocus = node("Developer", 200, [node("dev", 150)], { id: dev.id });
    const s = run(
      { type: "payload", payload: live },
      { type: "open", crumb: { id: docs.id, name: "Documents" } },
      { type: "payload", payload: { ...live, focus: devFocus } },
    );
    expect(screen(s).loading).toBe(true);
    expect(screen(s).rows).toEqual([]);
  });

  it("goes back up the breadcrumb", () => {
    const s = run(
      { type: "payload", payload: live },
      { type: "open", crumb: { id: dev.id, name: "Developer" } },
      { type: "up", to: -1 },
    );
    expect(screen(s).crumbs).toEqual([]);
    expect(screen(s).rows.map((r) => r.name)).toEqual(["Developer", "Documents"]);
  });

  it("keeps last time's size one level down too", () => {
    const cached = payload(
      node("Mac", 300, [node("Developer", 200, [node("dev", 120)]), node("Documents", 100)]),
      { running: false, finished: 90 },
    );
    const focus = node("Developer", 200, [node("dev", 30, [], { state: "measuring" })], { id: dev.id });
    const s = run(
      { type: "cached", payload: cached },
      { type: "payload", payload: live },
      { type: "open", crumb: { id: dev.id, name: "Developer" } },
      { type: "payload", payload: { ...live, focus } },
    );
    expect(screen(s).rows[0].previous).toBe(120);
  });

  it("stays in the same category when a new walk starts, by name", () => {
    const next = payload(node("Mac", 300, [node("Documents", 100), node("Developer", 10)]), {
      started: 200,
    });
    const newDev = next.root.children[1];
    const s = run(
      { type: "payload", payload: live },
      { type: "open", crumb: { id: dev.id, name: "Developer" } },
      { type: "open", crumb: { id: 999, name: "dev" } },
      { type: "payload", payload: next },
    );
    expect(screen(s).crumbs).toEqual([{ id: newDev.id, name: "Developer" }]);
  });
});
