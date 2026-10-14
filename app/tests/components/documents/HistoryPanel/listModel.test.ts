import { afterAll, beforeAll, describe, expect, test } from "bun:test";

import {
  dayHeading,
  fmtDelta,
  groupVersionsByDay,
  lengthDelta,
} from "@/components/documents/HistoryPanel/listModel";
import { pinDisplayLocale } from "@/lib/format/locale";
import type { DocumentVersion } from "@/lib/notes/documentVersions";

// The expected strings are Australian English.
pinDisplayLocale("en-AU");

// Days are local; pin the zone so the UTC stamps below fall on known days.
// Test files share a process, so put it back after.
const zone = process.env.TZ;
beforeAll(() => {
  process.env.TZ = "Australia/Melbourne";
});
afterAll(() => {
  if (zone === undefined) delete process.env.TZ;
  else process.env.TZ = zone;
});

let nextId = 1;
const version = (created_at: string, length: number): DocumentVersion => ({
  id: nextId++,
  file_id: 1,
  number: null,
  label: null,
  kind: "auto",
  hash: "",
  created_at,
  length,
});

describe("groupVersionsByDay", () => {
  test("splits a newest-first list at local midnight, keeping each row's index", () => {
    // AEDT is UTC+11: 13:30 UTC on the 12th is 00:30 on the 13th in Melbourne.
    const list = [
      version("2026-10-13 02:00:00", 30),
      version("2026-10-12 13:30:00", 20),
      version("2026-10-12 12:30:00", 10),
      version("2026-10-10 01:00:00", 5),
    ];
    const days = groupVersionsByDay(list);
    expect(days.map((d) => d.rows.map((r) => r.index))).toEqual([[0, 1], [2], [3]]);
    expect(new Date(days[0].day).getDate()).toBe(13);
    expect(new Date(days[1].day).getDate()).toBe(12);
    expect(new Date(days[2].day).getDate()).toBe(10);
  });

  test("a row whose stamp doesn't parse joins the day above it", () => {
    const list = [version("2026-10-13 02:00:00", 1), version("garbage", 1)];
    expect(groupVersionsByDay(list).map((d) => d.rows.length)).toEqual([2]);
  });

  test("an empty list has no days", () => {
    expect(groupVersionsByDay([])).toEqual([]);
  });
});

describe("dayHeading", () => {
  // Built inside each test, after the zone is pinned.
  const at = () => new Date(2026, 9, 13, 15, 0);
  test("today and yesterday by name", () => {
    const now = at();
    expect(dayHeading(new Date(2026, 9, 13).getTime(), now)).toBe("Today");
    expect(dayHeading(new Date(2026, 9, 12).getTime(), now)).toBe("Yesterday");
  });
  test("older days by weekday and date, with the year only when it differs", () => {
    const now = at();
    // en-AU puts a comma after the weekday, as `fmtDayHeading` writes it.
    expect(dayHeading(new Date(2026, 9, 8).getTime(), now)).toBe("Thu, 8 Oct");
    expect(dayHeading(new Date(2025, 11, 31).getTime(), now)).toBe("Wed, 31 Dec 2025");
  });
});

describe("lengthDelta", () => {
  const list = [version("2026-10-13 02:00:00", 120), version("2026-10-13 01:00:00", 83), version("2026-10-13 00:00:00", 83)];
  test("is the change over the next-older row", () => {
    expect(lengthDelta(list, 0)).toBe(37);
    expect(lengthDelta(list, 1)).toBe(0);
  });
  test("is null for the oldest and out of range", () => {
    expect(lengthDelta(list, 2)).toBeNull();
    expect(lengthDelta(list, 5)).toBeNull();
  });
});

describe("fmtDelta", () => {
  test("signs with a plus, a real minus, or ±0", () => {
    expect(fmtDelta(37)).toBe("+37");
    expect(fmtDelta(-12)).toBe("−12");
    expect(fmtDelta(0)).toBe("±0");
    expect(fmtDelta(1204)).toBe("+1,204");
  });
});
