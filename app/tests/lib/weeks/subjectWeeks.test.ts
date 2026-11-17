import { describe, expect, test } from "bun:test";
import { subjectWeeks, TEACHING_WEEK_THRESHOLD, weekOf, type SubjectWeeksInput } from "@/lib/weeks";
import { event, lecture, subject } from "./fixtures";

const derive = (extra: Partial<SubjectWeeksInput> = {}) => subjectWeeks({
  subjectId: 1, subjects: [subject()], classes: [], lectures: [],
  now: new Date("2026-09-23T12:00:00"), ...extra,
});
const labels = (extra: Partial<SubjectWeeksInput>) => derive(extra).weeks.map((week) => week.label);

describe("term-wide teaching weeks", () => {
  const subjects = [subject(1), subject(2), subject(3), subject(4), subject(5, "June intensive")];
  const classes = [1, 2, 3, 4].flatMap((id) => [
    event("2026-09-14T11:00:00", id),
    ...(id === 3 ? [] : [event("2026-09-21T11:00:00", id)]),
    event("2026-10-05T11:00:00", id),
  ]);

  test("the term break survives recordings and another term's classes", () => {
    const strip = derive({ subjects, classes: [...classes, event("2026-06-01T11:00:00", 5), event("2026-09-28T11:00:00", 5)],
      lectures: [lecture("2026-09-28T11:05:00")], now: new Date("2026-09-30T12:00:00") });
    expect(strip.weeks.map((week) => [week.label, week.number, week.kind])).toEqual([
      ["1", 1, "teaching"], ["2", 2, "teaching"], ["Break", null, "break"],
      ["3", 3, "teaching"], ["+1", null, "after"], ["+2", null, "after"], ["+3", null, "after"],
    ]);
    expect(strip.current).toBe(2);
    expect(strip.source).toBe("term");
  });

  test("a subject's public holiday does not change the term's week number", () => {
    expect(labels({ subjectId: 3, subjects, classes })).toEqual(["1", "2", "Break", "3", "+1", "+2", "+3"]);
  });

  test("a lone late event cannot extend the strip, even without named lectures", () => {
    const unnamed = classes.map((row) => ({ ...row, title: "Class" }));
    const normal = derive({ subjects, classes: unnamed });
    const outlier = derive({ subjects, classes: [...unnamed, event("2026-11-15T11:00:00", 1, { title: "Give feedback to group members" })] });
    expect(outlier.weeks).toEqual(normal.weeks);
    expect(outlier.weeks[outlier.weeks.length - 1].start).toEqual(new Date("2026-10-26T00:00:00"));
  });

  test("single-subject terms have breaks and prefer named lectures", () => {
    expect(labels({ classes: [
      event("2026-09-21T11:00:00"), event("2026-10-05T11:00:00", 1, { title: "LECTURES" }),
      event("2026-09-28T11:00:00", 1, { title: "Tutorial" }),
      event("2026-11-15T11:00:00", 1, { title: "Feedback" }),
      event("2026-09-28T11:00:00", 1, { kind: "due" }),
      event("2026-09-28T11:00:00", 1, { all_day: 1 }),
    ] })).toEqual(["1", "Break", "2", "+1", "+2", "+3"]);
  });

  test("title preference belongs to each subject, not the whole term", () => {
    expect(labels({ subjects: [subject(1), subject(2)], classes: [
      event("2026-09-21T11:00:00", 1), event("2026-10-05T11:00:00", 1),
      event("2026-09-28T11:00:00", 2, { title: "Workshop" }),
    ] })).toEqual(["1", "2", "3", "+1", "+2", "+3"]);
  });

  test("two-subject terms need one subject; subjects without classes do not raise the threshold", () => {
    expect(TEACHING_WEEK_THRESHOLD).toBe(0.5);
    expect(labels({ subjects: [subject(1), subject(2), subject(3), subject(4), subject(5)], classes: [
      event("2026-09-21T11:00:00", 2), event("2026-09-28T11:00:00", 1), event("2026-10-05T11:00:00", 1),
    ] })).toEqual(["1", "2", "3", "+1", "+2", "+3"]);
  });

  test("odd thresholds round up and repeated events count only one subject", () => {
    expect(labels({ subjects: [subject(1), subject(2), subject(3)], classes: [
      event("2026-09-21T11:00:00", 1), event("2026-09-21T11:00:00", 2), event("2026-09-21T11:00:00", 3),
      event("2026-09-28T11:00:00", 1), event("2026-09-29T11:00:00", 1), event("2026-09-30T11:00:00", 1),
      event("2026-10-05T11:00:00", 1), event("2026-10-05T11:00:00", 2),
    ] })).toEqual(["1", "Break", "2", "+1", "+2", "+3"]);
  });

  test("early-ending subjects share week 1 but start their after weeks earlier", () => {
    const input = { subjects: [subject(1), subject(2)], classes: [
      event("2026-09-14T11:00:00", 2), event("2026-09-21T11:00:00", 1),
      event("2026-09-28T11:00:00", 2), event("2026-10-05T11:00:00", 2),
    ] };
    expect(labels(input)).toEqual(["1", "2", "+1", "+2", "+3"]);
    expect(labels({ ...input, subjectId: 2 })).toEqual(["1", "2", "3", "4", "+1", "+2", "+3"]);
  });

  test("a recording-only subject inherits future term weeks", () => {
    expect(labels({ subjects: [...subjects, subject(6)], subjectId: 6, classes,
      lectures: [lecture("2026-08-01T11:05:00", 6)],
    })).toEqual(["1", "2", "Break", "3", "+1", "+2", "+3"]);
  });

  test("no timetable means continuous weeks from this subject's recordings only", () => {
    const strip = derive({ subjects: [subject(1), subject(2), subject(3, "Other term")], classes: [
      event("2026-01-01T12:00:00", 1, { kind: "due" }),
      event("2026-01-01T12:00:00", 2, { all_day: 1 }),
      event("2026-01-01T12:00:00", 3),
      event("2026-01-01T12:00:00", 1, { start_at: "bad" }),
    ], lectures: [lecture("2026-10-05T11:00:00"), lecture("2026-09-21T11:00:00"), lecture("bad"), lecture("2026-01-01T12:00:00", 2)] });
    expect(strip.source).toBe("recordings");
    expect(strip.weeks.map((week) => week.label)).toEqual(["1", "2", "3", "+1", "+2", "+3"]);
  });

  test("no valid dates or no qualifying teaching week produces an empty strip", () => {
    expect(derive({ lectures: [lecture("bad")] })).toEqual({ weeks: [], current: -1, source: "recordings" });
    expect(derive({ subjects: [subject(1), subject(2), subject(3)], classes: [
      event("2026-09-07T11:00:00", 1), event("2026-09-14T11:00:00", 2), event("2026-09-21T11:00:00", 3),
    ], lectures: [lecture("2026-09-07T11:05:00")] })).toEqual({ weeks: [], current: -1, source: "term" });
  });
});

describe("week assignment", () => {
  const input = { classes: [event("2026-09-21T11:00:00"), event("2026-10-05T11:00:00")] };

  test("now and activity clamp before week 1 and after +3", () => {
    const before = derive({ ...input, now: new Date("2026-01-01T00:00:00") });
    const after = derive({ ...input, now: new Date("2028-01-01T00:00:00") });
    expect(before.current).toBe(0);
    expect(after.current).toBe(after.weeks.length - 1);
    expect(weekOf(before, new Date("2026-01-01T00:00:00"))).toBe(before.weeks[0].key);
    expect(weekOf(after, new Date("2028-01-01T00:00:00"))).toBe(after.weeks[after.current].key);
    expect(after.weeks[after.current].label).toBe("+3");
  });

  test("weeks have exclusive next-Monday ends and Sunday stays in its week", () => {
    const strip = derive(input);
    expect(weekOf(strip, new Date("2026-09-27T23:59:59"))).toBe(strip.weeks[0].key);
    expect(weekOf(strip, new Date("2026-09-28T00:00:00"))).toBe(strip.weeks[1].key);
    for (let i = 0; i < strip.weeks.length; i++) {
      expect(strip.weeks[i].start.getDay()).toBe(1);
      expect(strip.weeks[i].start.getHours()).toBe(0);
      expect(strip.weeks[i].end.getDay()).toBe(1);
      expect(strip.weeks[i].end.getHours()).toBe(0);
      if (i > 0) expect(strip.weeks[i - 1].end).toEqual(strip.weeks[i].start);
    }
  });

  test("empty strips and invalid dates have no assignment", () => {
    expect(weekOf(derive(), new Date())).toBeNull();
    expect(weekOf(derive(input), new Date("bad"))).toBeNull();
  });
});
