import type { Submission, SubmissionTestcase } from "@/lib/ed/workspace";

/**
 * Ed's feedback panel, read from a submission: the "N / M passed, P points"
 * summary and each `check_diff` line grouped into runs to draw.
 */

export interface SubmissionSummary {
  passed: number;
  total: number;
  points: number;
}

/** Ed's counts when it gave them, else counted from the testcases; points
 *  are the testcases' scores summed. */
export function submissionSummary(sub: Submission): SubmissionSummary {
  const cases = sub.result?.testcases ?? [];
  const points = cases.reduce((sum, tc) => sum + (tc.score ?? 0), 0);
  return {
    passed: sub.testcasePassCount ?? cases.filter((tc) => tc.passed).length,
    total: sub.testcaseTotalCount ?? cases.length,
    points: Math.round(points * 100) / 100,
  };
}

/** "2 / 3 passed, 1.5 points". */
export function summaryText({ passed, total, points }: SubmissionSummary): string {
  return `${passed} / ${total} passed, ${points} ${points === 1 ? "point" : "points"}`;
}

/** Hidden and private testcases show their name only. */
export const testcaseShown = (tc: SubmissionTestcase) => !tc.hidden && !tc.private;

export type DiffRunKind = "observed" | "expected" | "same" | "missingNewline";

export interface DiffRun {
  kind: DiffRunKind;
  text: string;
}

const KINDS: Record<string, DiffRunKind> = { "-": "observed", "+": "expected", "=": "same", mn: "missingNewline" };

/**
 * Each diff line as runs to draw, neighbours of one kind merged. A run with no
 * text (Ed opens a line with its sign alone) is dropped unless it marks a
 * missing newline, and an unknown sign draws as plain text.
 */
export function diffLines(diffs: ReadonlyArray<ReadonlyArray<readonly [string, string?]>>): DiffRun[][] {
  return diffs.map((line) => {
    const runs: DiffRun[] = [];
    for (const [sign, text] of line) {
      const kind = KINDS[sign] ?? "same";
      if (kind === "missingNewline") {
        runs.push({ kind, text: "" });
        continue;
      }
      if (!text) continue;
      const last = runs[runs.length - 1];
      if (last && last.kind === kind) last.text += text;
      else runs.push({ kind, text });
    }
    return runs;
  });
}
