import { describe, expect, test } from "bun:test";

import {
  ageLabel,
  checklistProgress,
  estimateLabel,
  fileFolder,
  parseEstimate,
} from "@/components/projects/task/taskFormat";
import { pinDisplayLocale } from "@/lib/format/locale";

pinDisplayLocale("en-AU");

describe("estimate", () => {
  test("reads as minutes, or nothing when unset", () => {
    expect(estimateLabel(90)).toBe("90 min");
    expect(estimateLabel(5)).toBe("5 min");
    expect(estimateLabel(null)).toBeNull();
  });

  test("text becomes whole minutes", () => {
    expect(parseEstimate("90")).toBe(90);
    expect(parseEstimate(" 45 ")).toBe(45);
    expect(parseEstimate("12.6")).toBe(13);
  });

  test("empty or zero clears, nonsense leaves the value alone", () => {
    expect(parseEstimate("")).toBeNull();
    expect(parseEstimate("  ")).toBeNull();
    expect(parseEstimate("0")).toBeNull();
    expect(parseEstimate("abc")).toBeUndefined();
    expect(parseEstimate("-5")).toBeUndefined();
    expect(parseEstimate("Infinity")).toBeUndefined();
  });
});

describe("ageLabel", () => {
  const now = Date.parse("2026-10-15T12:00:00Z");
  const ago = (ms: number) => now - ms;
  const MIN = 60_000;
  const HOUR = 60 * MIN;
  const DAY = 24 * HOUR;

  test("minutes, hours and days", () => {
    expect(ageLabel(ago(10_000), now)).toBe("just now");
    expect(ageLabel(ago(5 * MIN), now)).toBe("5m ago");
    expect(ageLabel(ago(59 * MIN), now)).toBe("59m ago");
    expect(ageLabel(ago(3 * HOUR), now)).toBe("3h ago");
    expect(ageLabel(ago(23 * HOUR), now)).toBe("23h ago");
    expect(ageLabel(ago(3 * DAY), now)).toBe("3d ago");
    expect(ageLabel(ago(6 * DAY + 5 * HOUR), now)).toBe("6d ago");
  });

  test("a week or more is a date, not a count", () => {
    expect(ageLabel(ago(7 * DAY), now)).not.toMatch(/ago$/);
  });

  test("a clock a little ahead still reads as now", () => {
    expect(ageLabel(now + 2 * MIN, now)).toBe("just now");
  });
});

describe("checklistProgress", () => {
  test("counts done items and the percentage", () => {
    const items = [{ done_at: "2026-10-01 00:00:00" }, { done_at: null }, { done_at: null }];
    expect(checklistProgress(items)).toEqual({ done: 1, total: 3, pct: 33 });
  });

  test("an empty checklist is 0%, not NaN", () => {
    expect(checklistProgress([])).toEqual({ done: 0, total: 0, pct: 0 });
  });

  test("all done is 100", () => {
    expect(checklistProgress([{ done_at: "x" }, { done_at: "y" }]).pct).toBe(100);
  });
});

describe("fileFolder", () => {
  test("the folder under the subject", () => {
    expect(fileFolder("courses/MULT20015_2026_SM2/files/week-1/notes.pdf")).toBe("files/week-1");
    expect(fileFolder("courses/ABCD1_2026_SM1/documents/essay.md")).toBe("documents");
  });

  test("a file at the subject's top has no folder", () => {
    expect(fileFolder("courses/ABCD1_2026_SM1/syllabus.md")).toBe("");
  });
});
