import { describe, expect, it } from "vitest";
import { categoryColor, statusLine } from "./present";
import type { Screen } from "./state";
import { fmtDate } from "../format";

describe("categoryColor", () => {
  it("follows the category, not its size or position", () => {
    expect(categoryColor("Applications")).toBe(categoryColor("Applications"));
    expect(categoryColor("Applications")).not.toBe(categoryColor("Developer"));
  });

  it("keeps the big neighbours apart", () => {
    const big = ["Applications", "Developer", "Documents", "macOS", "System Data"];
    expect(new Set(big.map(categoryColor)).size).toBe(big.length);
  });

  it("has a muted fallback for anything unexpected", () => {
    expect(categoryColor("Something new")).toBe(categoryColor("Another"));
  });
});

const base: Screen = {
  title: "Macintosh HD",
  crumbs: [],
  total: 0,
  rows: [],
  other: null,
  running: false,
  preview: false,
  asOf: null,
  loading: false,
  files: 0,
  unreadable: 0,
};

describe("statusLine", () => {
  it("counts files while measuring", () => {
    expect(statusLine({ ...base, running: true, files: 1234567 })).toBe(
      `Measuring · ${(1234567).toLocaleString()} files`,
    );
  });

  it("says when a preview was taken and that it is being refreshed", () => {
    expect(statusLine({ ...base, preview: true, asOf: 1760000000 })).toBe(
      `As of ${fmtDate(1760000000)} · updating`,
    );
  });

  it("dates a finished walk", () => {
    expect(statusLine({ ...base, asOf: 1760000000 })).toBe(`Measured ${fmtDate(1760000000)}`);
  });

  it("is honest before anything is known", () => {
    expect(statusLine(base)).toBe("Not measured yet");
  });
});
