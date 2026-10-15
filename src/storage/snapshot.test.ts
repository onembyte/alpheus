import { describe, expect, it } from "vitest";
import { shownCards } from "./snapshot";
import type { Card } from "../types";

const card = (id: string): Card => ({
  id,
  title: id,
  description: "",
  tier: "safe",
  size_kb: 1,
  paths: [],
  proof: null,
  action: "delete",
  command_display: null,
});

describe("shownCards", () => {
  it("shows the last scan's cards until the new scan lands", () => {
    const snap = { cards: [card("old")], at: 100 };
    expect(shownCards(null, snap)).toEqual({ cards: [card("old")], fromSnapshot: true, at: 100 });
  });

  it("switches to the new scan as soon as it lands, even if it found nothing", () => {
    const snap = { cards: [card("old")], at: 100 };
    expect(shownCards([card("new")], snap)).toEqual({
      cards: [card("new")],
      fromSnapshot: false,
      at: null,
    });
    expect(shownCards([], snap).cards).toEqual([]);
  });

  it("has nothing to show on a first launch", () => {
    expect(shownCards(null, null)).toEqual({ cards: null, fromSnapshot: false, at: null });
  });
});
