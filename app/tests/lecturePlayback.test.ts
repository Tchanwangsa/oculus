import { describe, expect, test } from "bun:test";
import { spanAt } from "../src/lib/lectures";

describe("chronological playback spans", () => {
  test("selects the last started entry, including exact boundaries and duplicate starts", () => {
    const starts = [0, 12, 12, 40];
    expect(spanAt(starts, -1)).toBe(-1);
    expect(spanAt(starts, 0)).toBe(0);
    expect(spanAt(starts, 11.99)).toBe(0);
    expect(spanAt(starts, 12)).toBe(2);
    expect(spanAt(starts, 39.99)).toBe(2);
    expect(spanAt(starts, 40)).toBe(3);
    expect(spanAt(starts, Infinity)).toBe(3);
    expect(spanAt(starts, NaN)).toBe(-1);
    expect(spanAt([], 10)).toBe(-1);
  });
});
