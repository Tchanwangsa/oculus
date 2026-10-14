import { expect, test } from "bun:test";
import { displayCode, fmtRelativeDay, sqliteUtcToMs } from "@/lib/format/format";

test("a relative day is Today, Yesterday, a weekday, then a date", () => {
  const now = new Date(2026, 9, 14, 15);
  const day = (d: number, m = 9, y = 2026) => new Date(y, m, d, 9).getTime();
  expect(fmtRelativeDay(day(14), now)).toBe("Today");
  expect(fmtRelativeDay(day(13), now)).toBe("Yesterday");
  expect(fmtRelativeDay(day(11), now)).toBe("Sunday");
  // The date's word order follows the locale; only the year differs by age.
  expect(fmtRelativeDay(day(5), now)).toMatch(/Oct/);
  expect(fmtRelativeDay(day(5), now)).not.toContain("2026");
  expect(fmtRelativeDay(day(5, 8, 2025), now)).toContain("2025");
});

test("shared timestamp conversion treats SQLite UTC and explicit ISO zones consistently", () => {
  expect(sqliteUtcToMs("2026-10-02 09:10:00")).toBe(sqliteUtcToMs("2026-10-02T09:10:00Z"));
  expect(sqliteUtcToMs("2026-10-02T19:10:00+10:00")).toBe(sqliteUtcToMs("2026-10-02T09:10:00Z"));
  expect(sqliteUtcToMs("invalid")).toBeUndefined();
});

test("a code loses only a year-led term suffix", () => {
  expect(displayCode("MULT20015_2026_SM2")).toBe("MULT20015");
  expect(displayCode("CS_101_2026_FA")).toBe("CS_101");
  expect(displayCode("CS_101")).toBe("CS_101");
  expect(displayCode("CS 101-001")).toBe("CS 101-001");
  expect(displayCode("INFO30006")).toBe("INFO30006");
});
