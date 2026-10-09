import { describe, expect, test } from "bun:test";
import {
  CHART_DAYS,
  NO_SUBJECT_SERIES,
  OTHER_SUBJECTS,
  bestStreak,
  chartDays,
  createThrottle,
  dayKey,
  fmtUsage,
  hourStep,
  percentChange,
  perDayUsed,
  rangeTotals,
  stackBySubject,
  stackByType,
  typeGroup,
  usageStreak,
  type UsageContextRow,
  type UsageDay,
} from "@/lib/activity/usage";

const day = (y: number, m: number, d: number) => new Date(y, m - 1, d);
// Tuesday 6 Oct 2026.
const today = day(2026, 10, 6);

const days = (entries: [Date, number][]) =>
  new Map<string, UsageDay>(entries.map(([d, active]) => [dayKey(d), { open: active * 2, active }]));

describe("chart window", () => {
  test("30 days, oldest first, ending today", () => {
    const range = chartDays(today);
    expect(range).toHaveLength(CHART_DAYS);
    expect(range[0].key).toBe("2026-09-07");
    expect(range[CHART_DAYS - 1].key).toBe("2026-10-06");
    expect(range.map((d) => d.key)).toEqual([...range.map((d) => d.key)].sort());
  });

  test("windowsBack ends the day before the window after it starts", () => {
    const prior = chartDays(today, 1);
    expect(prior).toHaveLength(CHART_DAYS);
    expect(prior[0].key).toBe("2026-08-08");
    expect(prior[CHART_DAYS - 1].key).toBe("2026-09-06");
    expect(chartDays(today)[0].key).toBe("2026-09-07");
  });

  test("calendar days across a DST change", () => {
    // Sydney leaves standard time on 4 Oct 2026; every key still appears once.
    const keys = chartDays(today).map((d) => d.key);
    expect(new Set(keys).size).toBe(CHART_DAYS);
    expect(keys).toContain("2026-10-04");
  });
});

describe("range totals", () => {
  test("sum the range and count days with active time", () => {
    const m = days([[day(2026, 10, 6), 60], [day(2026, 10, 1), 30], [day(2026, 9, 1), 500]]);
    m.set(dayKey(day(2026, 10, 2)), { open: 100, active: 0 });
    expect(rangeTotals(m, chartDays(today))).toEqual({ open: 280, active: 90, daysUsed: 2 });
  });

  test("per day used averages over used days only", () => {
    expect(perDayUsed({ active: 90, daysUsed: 2 })).toBe(45);
    expect(perDayUsed({ active: 0, daysUsed: 0 })).toBe(0);
  });

  test("percent change is whole and null with nothing before", () => {
    expect(percentChange(150, 100)).toBe(50);
    expect(percentChange(50, 100)).toBe(-50);
    expect(percentChange(1, 3)).toBe(-67);
    expect(percentChange(10, 0)).toBeNull();
  });
});

describe("gridlines and durations", () => {
  test("the smallest hour step that fits in four lines", () => {
    expect(hourStep(0)).toBe(0.5);
    expect(hourStep(2 * 3600)).toBe(0.5);
    expect(hourStep(2 * 3600 + 1)).toBe(1);
    expect(hourStep(12 * 3600)).toBe(3);
    expect(hourStep(24 * 3600)).toBe(6);
    expect(hourStep(30 * 3600)).toBe(8);
  });

  test("durations read as minutes, then hours and minutes", () => {
    expect(fmtUsage(0)).toBe("0m");
    expect(fmtUsage(20)).toBe("<1m");
    expect(fmtUsage(45 * 60)).toBe("45m");
    expect(fmtUsage(3 * 3600)).toBe("3h");
    expect(fmtUsage(2 * 3600 + 15 * 60)).toBe("2h 15m");
    expect(fmtUsage(12 * 3600 + 20 * 60)).toBe("12h");
  });
});

describe("streaks", () => {
  test("counts back from today when today is active", () => {
    const m = days([[day(2026, 10, 6), 60], [day(2026, 10, 5), 60], [day(2026, 10, 4), 60], [day(2026, 10, 2), 60]]);
    expect(usageStreak(m, today)).toBe(3);
  });

  test("counts back from yesterday when today has nothing yet", () => {
    const m = days([[day(2026, 10, 5), 60], [day(2026, 10, 4), 60]]);
    expect(usageStreak(m, today)).toBe(2);
  });

  test("open-only days and gaps break it", () => {
    const m = days([[day(2026, 10, 5), 0], [day(2026, 10, 4), 60]]);
    expect(usageStreak(m, today)).toBe(0);
    expect(usageStreak(new Map(), today)).toBe(0);
  });

  test("runs across a month boundary", () => {
    const m = days([[day(2026, 10, 1), 60], [day(2026, 9, 30), 60], [day(2026, 9, 29), 60]]);
    expect(usageStreak(m, day(2026, 10, 1))).toBe(3);
  });

  test("the best run anywhere, open-only days excluded", () => {
    const m = days([
      [day(2026, 1, 30), 60], [day(2026, 1, 31), 60], [day(2026, 2, 1), 60], [day(2026, 2, 2), 60],
      [day(2026, 2, 3), 0],
      [day(2026, 10, 5), 60], [day(2026, 10, 6), 60],
    ]);
    expect(bestStreak(m)).toBe(4);
    expect(bestStreak(new Map())).toBe(0);
  });
});

describe("active time by type", () => {
  const row = (d: string, kind: UsageContextRow["kind"], active: number, subjectId = 0): UsageContextRow =>
    ({ day: d, kind, subjectId, active });
  const range = chartDays(today);

  test("kinds fold into display groups; unknown kinds are other", () => {
    expect(typeGroup("lecture")).toBe("lectures");
    expect(typeGroup("file")).toBe("files");
    expect(typeGroup("document")).toBe("files");
    expect(typeGroup("course")).toBe("course");
    expect(typeGroup("chat")).toBe("chat");
    expect(typeGroup("browser")).toBe("browser");
    expect(typeGroup("planning")).toBe("other");
    expect(typeGroup("other")).toBe("other");
    expect(typeGroup("something-new")).toBe("other");
  });

  test("fixed group order, summed per day, only groups with time", () => {
    const s = stackByType([
      row("2026-10-06", "planning", 10),
      row("2026-10-06", "document", 20),
      row("2026-10-06", "file", 30, 4),
      row("2026-10-05", "lecture", 100, 4),
      row("2026-10-05", "chat", 0),
      row("2026-08-01", "browser", 999),
    ], range);
    expect(s.series).toEqual(["lectures", "files", "other"]);
    expect(s.days.get("2026-10-06")).toEqual([0, 50, 10]);
    expect(s.days.get("2026-10-05")).toEqual([100, 0, 0]);
    expect(s.days.has("2026-08-01")).toBe(false);
    expect(s.max).toBe(100);
  });

  test("no rows is an empty chart", () => {
    expect(stackByType([], range)).toEqual({ series: [], days: new Map(), max: 0 });
  });
});

describe("active time by subject", () => {
  const row = (d: string, subjectId: number, active: number): UsageContextRow =>
    ({ day: d, kind: "course", subjectId, active });
  const range = chartDays(today);

  test("subjects by id, then no subject; kinds summed", () => {
    const s = stackBySubject([
      row("2026-10-06", 0, 5),
      row("2026-10-06", 30, 10),
      row("2026-10-06", 7, 20),
      { day: "2026-10-06", kind: "lecture", subjectId: 7, active: 1 },
    ], range);
    expect(s.series).toEqual(["7", "30", NO_SUBJECT_SERIES]);
    expect(s.days.get("2026-10-06")).toEqual([21, 10, 5]);
    expect(s.max).toBe(36);
  });

  test("past five subjects the smallest fold into other subjects", () => {
    const rows = [1, 2, 3, 4, 5, 6, 7].map((id) => row("2026-10-06", id, id * 10));
    rows.push(row("2026-10-05", 1, 1000), row("2026-10-05", 0, 3));
    const s = stackBySubject(rows, range);
    // 1 is the largest over the window; 2 and 3 are the smallest.
    expect(s.series).toEqual(["1", "4", "5", "6", "7", OTHER_SUBJECTS, NO_SUBJECT_SERIES]);
    expect(s.days.get("2026-10-06")).toEqual([10, 40, 50, 60, 70, 50, 0]);
    expect(s.days.get("2026-10-05")).toEqual([1000, 0, 0, 0, 0, 0, 3]);
  });

  test("ties fold the higher id; days outside the window don't count", () => {
    const rows = [row("2026-10-06", 9, 10), row("2026-10-06", 8, 10), row("2026-08-01", 9, 999)];
    expect(stackBySubject(rows, range, 1).series).toEqual(["8", OTHER_SUBJECTS]);
  });
});

describe("ping throttle", () => {
  test("passes the first call, then once per interval", () => {
    let t = 1_000;
    const gate = createThrottle(30_000, () => t);
    expect(gate()).toBe(true);
    t += 29_999;
    expect(gate()).toBe(false);
    t += 1;
    expect(gate()).toBe(true);
    expect(gate()).toBe(false);
  });
});
