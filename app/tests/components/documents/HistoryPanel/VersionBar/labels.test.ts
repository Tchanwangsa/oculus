import { describe, expect, test } from "bun:test";

import { changesLabel, compareLabel, wordStats } from "@/components/documents/HistoryPanel/VersionBar/labels";
import { pinDisplayLocale } from "@/lib/format/locale";

pinDisplayLocale("en-AU");

describe("changesLabel", () => {
  test("counts from the current hunk, which counts from 0", () => {
    expect(changesLabel({ hunks: 12, current: 2 })).toBe("3 of 12 changes");
    expect(changesLabel({ hunks: 1, current: 0 })).toBe("1 of 1 change");
  });
  test("the total before one is picked, or none", () => {
    expect(changesLabel({ hunks: 12, current: null })).toBe("12 changes");
    expect(changesLabel({ hunks: 1, current: null })).toBe("1 change");
    expect(changesLabel({ hunks: 0, current: null })).toBe("No changes");
  });
});

describe("wordStats", () => {
  test("a plus and a real minus, grouped", () => {
    expect(wordStats({ inserted: 412, deleted: 38 })).toEqual({ inserted: "+412", deleted: "−38" });
    expect(wordStats({ inserted: 1500, deleted: 0 })).toEqual({ inserted: "+1,500", deleted: "−0" });
  });
});

describe("compareLabel", () => {
  test("names the base, or says the marks are off", () => {
    expect(compareLabel(true, "previous")).toBe("vs previous");
    expect(compareLabel(true, "current")).toBe("vs current note");
    expect(compareLabel(false, "previous")).toBe("Changes hidden");
  });
});
