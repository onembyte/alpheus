import { describe, expect, it } from "vitest";
import { accessNotice, cardLinks } from "./links";
import type { Card } from "../types";

function card(id: string, size_kb: number, paths: string[], extra: Partial<Card> = {}): Card {
  return {
    id,
    title: id,
    description: "",
    tier: "safe",
    size_kb,
    paths,
    proof: null,
    action: "delete",
    command_display: null,
    ...extra,
  };
}

describe("cardLinks", () => {
  it("links a card to every row that holds all of its paths", () => {
    const caches = card("library-caches", 900, ["/a", "/b"]);
    const links = cardLinks([caches], { "/a": [0, 2, 5, 7], "/b": [0, 2, 5, 9] });
    expect(links.get(2)?.cards).toEqual([caches]);
    expect(links.get(5)?.kb).toBe(900);
    expect(links.has(7)).toBe(false);
    expect(links.has(0)).toBe(false);
  });

  it("adds up several cards in one row, biggest first", () => {
    const a = card("a", 100, ["/x/1"]);
    const b = card("b", 300, ["/x/2"]);
    const links = cardLinks([a, b], { "/x/1": [0, 3, 4], "/x/2": [0, 3, 6] });
    expect(links.get(3)?.kb).toBe(400);
    expect(links.get(3)?.cards.map((c) => c.id)).toEqual(["b", "a"]);
  });

  it("skips cards that free nothing or reach outside the walk", () => {
    const info = card("os-update", 500, ["/p"], { action: "explain" });
    const outside = card("pacman", 500, ["/p", "/var/cache"]);
    const pathless = card("tm-snapshots", 0, []);
    const links = cardLinks([info, outside, pathless], { "/p": [0, 1, 2], "/var/cache": [] });
    expect(links.size).toBe(0);
  });
});

describe("accessNotice", () => {
  it("is silent when every folder could be read", () => {
    expect(accessNotice(0)).toBeNull();
  });

  it("counts the folders macOS kept private", () => {
    expect(accessNotice(195)).toContain("195 folders");
    expect(accessNotice(1)).toContain("1 folder ");
  });
});
