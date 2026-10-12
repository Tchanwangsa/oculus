import { describe, expect, test } from "bun:test";

import { diffLines, submissionSummary, summaryText, testcaseShown } from "@/lib/ed/feedback";
import type { Submission, SubmissionTestcase } from "@/lib/ed/workspace";

const tc = (over: Partial<SubmissionTestcase>): SubmissionTestcase => ({
  name: "t",
  description: null,
  hidden: false,
  private: false,
  score: null,
  maxScore: null,
  passed: false,
  skip: false,
  feedback: null,
  output: null,
  diffs: [],
  ...over,
});

const sub = (testcases: SubmissionTestcase[], counts: [number | null, number | null] = [null, null]): Submission => ({
  id: 1,
  status: "failed",
  createdAt: "2026-10-13T00:00:00Z",
  markedAt: null,
  testcasePassCount: counts[0],
  testcaseTotalCount: counts[1],
  feedback: null,
  result: { ok: true, passed: false, feedback: null, build: null, testcases },
});

describe("submission summary", () => {
  test("Ed's counts win, points sum the scores", () => {
    const s = sub([tc({ score: 1, passed: true }), tc({ score: 0.5 }), tc({ score: null })], [1, 3]);
    expect(submissionSummary(s)).toEqual({ passed: 1, total: 3, points: 1.5 });
    expect(summaryText(submissionSummary(s))).toBe("1 / 3 passed, 1.5 points");
  });

  test("counts from the testcases when Ed gave none", () => {
    const s = sub([tc({ passed: true, score: 1 }), tc({})]);
    expect(submissionSummary(s)).toEqual({ passed: 1, total: 2, points: 1 });
    expect(summaryText({ passed: 0, total: 1, points: 0 })).toBe("0 / 1 passed, 0 points");
    expect(summaryText({ passed: 1, total: 1, points: 1 })).toBe("1 / 1 passed, 1 point");
  });

  test("no result is zero of zero", () => {
    expect(submissionSummary({ ...sub([]), result: null })).toEqual({ passed: 0, total: 0, points: 0 });
  });

  test("hidden and private testcases show their name only", () => {
    expect(testcaseShown(tc({}))).toBe(true);
    expect(testcaseShown(tc({ hidden: true }))).toBe(false);
    expect(testcaseShown(tc({ private: true }))).toBe(false);
  });
});

describe("diff lines", () => {
  test("runs keep their kind and neighbours merge", () => {
    expect(
      diffLines([
        [["=", "Count: "], ["-", "4"], ["-", "2"], ["+", "5"]],
        [["=", "done"], ["mn", ""]],
      ]),
    ).toEqual([
      [
        { kind: "same", text: "Count: " },
        { kind: "observed", text: "42" },
        { kind: "expected", text: "5" },
      ],
      [{ kind: "same", text: "done" }, { kind: "missingNewline", text: "" }],
    ]);
  });

  test("a sign without text is dropped, an unknown sign is plain", () => {
    expect(diffLines([[["-"], ["-", "x"], ["?", "y"]]])).toEqual([
      [
        { kind: "observed", text: "x" },
        { kind: "same", text: "y" },
      ],
    ]);
  });
});
