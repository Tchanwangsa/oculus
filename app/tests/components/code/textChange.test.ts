import { describe, expect, test } from "bun:test";

import { textChange } from "@/components/code/textChange";

const apply = (before: string, after: string) => {
  const { from, to, insert } = textChange(before, after);
  return before.slice(0, from) + insert + before.slice(to);
};

describe("text change", () => {
  test("replaces only the span between the common start and end", () => {
    expect(textChange("hello world", "hello brave world")).toEqual({ from: 6, to: 6, insert: "brave " });
    expect(textChange("abcdef", "abXYef")).toEqual({ from: 2, to: 4, insert: "XY" });
    expect(textChange("same", "same")).toEqual({ from: 4, to: 4, insert: "" });
  });

  test("repeated characters at the seam never overlap the two ends", () => {
    for (const [a, b] of [["aaa", "aaaa"], ["aaaa", "aa"], ["", "x"], ["x", ""], ["abab", "ab"]]) {
      expect(apply(a, b)).toBe(b);
      const { from, to } = textChange(a, b);
      expect(from).toBeLessThanOrEqual(to);
    }
  });
});
