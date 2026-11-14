import { describe, expect, test } from "bun:test";

import { insertRankedSearchResult, scoreQueryMatch, scoreSubsequenceMatch } from "@/lib/search/ranking";

// T3 Code's `searchRanking.test.ts` (410177c7), as fixtures for the port.
describe("scoreQueryMatch", () => {
  const tiers = { exactBase: 0, prefixBase: 10, boundaryBase: 20, includesBase: 30, boundaryMarkers: ["-"] };

  test("treats boundary matches as stronger than generic contains matches", () => {
    const boundaryScore = scoreQueryMatch({ value: "gh-fix-ci", query: "fix", ...tiers });
    const containsScore = scoreQueryMatch({ value: "highfixci", query: "fix", ...tiers });
    expect(boundaryScore).not.toBeNull();
    expect(containsScore).not.toBeNull();
    expect(boundaryScore!).toBeLessThan(containsScore!);
  });

  test("ranks exact, prefix, boundary, includes, then fuzzy", () => {
    const all = { ...tiers, fuzzyBase: 100 };
    const exact = scoreQueryMatch({ value: "fix", query: "fix", ...all })!;
    const prefix = scoreQueryMatch({ value: "fixer", query: "fix", ...all })!;
    const boundary = scoreQueryMatch({ value: "gh-fix", query: "fix", ...all })!;
    const includes = scoreQueryMatch({ value: "ghfix", query: "fix", ...all })!;
    const fuzzy = scoreQueryMatch({ value: "f-i-x-y", query: "fix", ...all, boundaryMarkers: ["_"] })!;
    expect([exact, prefix, boundary, includes, fuzzy]).toEqual([...[exact, prefix, boundary, includes, fuzzy]].sort((a, b) => a - b));
    expect(new Set([exact, prefix, boundary, includes, fuzzy]).size).toBe(5);
  });

  test("an untried tier never matches", () => {
    expect(scoreQueryMatch({ value: "abc", query: "b", exactBase: 0 })).toBeNull();
    expect(scoreQueryMatch({ value: "", query: "b", exactBase: 0, includesBase: 1 })).toBeNull();
  });
});

describe("scoreSubsequenceMatch", () => {
  test("scores tighter subsequences ahead of looser ones", () => {
    const compact = scoreSubsequenceMatch("ghfixci", "gfc");
    const spread = scoreSubsequenceMatch("github-fix-ci", "gfc");
    expect(compact).not.toBeNull();
    expect(spread).not.toBeNull();
    expect(compact!).toBeLessThan(spread!);
  });

  test("is null when a letter is missing", () => {
    expect(scoreSubsequenceMatch("abc", "abd")).toBeNull();
  });
});

describe("insertRankedSearchResult", () => {
  test("keeps the best-ranked candidates within the limit", () => {
    const ranked = [
      { item: "b", score: 20, tieBreaker: "b" },
      { item: "d", score: 40, tieBreaker: "d" },
    ];
    insertRankedSearchResult(ranked, { item: "a", score: 10, tieBreaker: "a" }, 2);
    insertRankedSearchResult(ranked, { item: "c", score: 30, tieBreaker: "c" }, 2);
    expect(ranked.map((entry) => entry.item)).toEqual(["a", "b"]);
  });

  test("breaks a tie on the tie breaker", () => {
    const ranked: { item: string; score: number; tieBreaker: string }[] = [];
    insertRankedSearchResult(ranked, { item: "z", score: 1, tieBreaker: "z" }, Infinity);
    insertRankedSearchResult(ranked, { item: "a", score: 1, tieBreaker: "a" }, Infinity);
    expect(ranked.map((entry) => entry.item)).toEqual(["a", "z"]);
  });
});
