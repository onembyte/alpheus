import { describe, expect, it } from "vitest";
import { latestOnly } from "./latest";

describe("latestOnly", () => {
  it("applies only the newest of a burst, once", () => {
    const applied: number[] = [];
    const pending: (() => void)[] = [];
    const push = latestOnly<number>((n) => applied.push(n), (flush) => pending.push(flush));
    push(1);
    push(2);
    push(3);
    expect(applied).toEqual([]);
    expect(pending.length).toBe(1);
    pending.shift()!();
    expect(applied).toEqual([3]);
  });

  it("schedules again for what arrives after a flush", () => {
    const applied: number[] = [];
    const pending: (() => void)[] = [];
    const push = latestOnly<number>((n) => applied.push(n), (flush) => pending.push(flush));
    push(1);
    pending.shift()!();
    push(2);
    expect(pending.length).toBe(1);
    pending.shift()!();
    expect(applied).toEqual([1, 2]);
  });
});
