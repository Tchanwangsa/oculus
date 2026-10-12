import { describe, expect, test } from "bun:test";
import type { DbCalendarEvent, Lecture } from "@/lib/db";
import { groupLecturesByWeek } from "@/lib/lectures/weeks";

const lecture = (date: string, id = date) => ({ id, date, subject_id: 1 }) as Lecture;
const event = (date: string, title = "Lecture", extra: Partial<DbCalendarEvent> = {}) => ({
  subject_id: 1, kind: "class", title, start_at: new Date(date).toISOString(), all_day: 0, ...extra,
}) as DbCalendarEvent;
const shape = (lectures: Lecture[], calendar: DbCalendarEvent[] = []) =>
  groupLecturesByWeek(lectures, calendar).map((group) => [group.week, group.isBreak, group.lectures.map((row) => row.id)]);

describe("lecture teaching weeks", () => {
  test("a capture inside a timetable gap stays in a break, and teaching numbers resume", () => {
    const rows = [lecture("2026-09-21T12:05:00", "before"), lecture("2026-09-30T09:05:00", "break"), lecture("2026-10-05T12:05:00", "after")];
    expect(shape(rows, [event("2026-09-21T12:00:00"), event("2026-10-05T12:00:00")])).toEqual([
      [1, false, ["before"]], [null, true, ["break"]], [2, false, ["after"]],
    ]);
  });

  test("an empty break appears, while unrelated deadlines, subjects and tutorials do not fill it", () => {
    const rows = [lecture("2026-09-21T12:05:00", "before"), lecture("2026-10-05T12:05:00", "after")];
    expect(shape(rows, [
      event("2026-09-21T12:00:00"), event("2026-10-05T12:00:00"),
      event("2026-09-30T12:00:00", "Assignment", { kind: "due" }),
      event("2026-09-30T12:00:00", "Lecture", { subject_id: 2 }),
      event("2026-09-30T12:00:00", "Tutorial"),
      event("2026-09-30T12:00:00", "Announcement", { all_day: 1 }),
    ])).toEqual([[1, false, ["before"]], [null, true, []], [2, false, ["after"]]]);
  });

  test("missing recordings with scheduled classes still count as a teaching week", () => {
    const rows = [lecture("2026-09-21T12:05:00", "before"), lecture("2026-10-05T12:05:00", "after")];
    expect(shape(rows, [event("2026-09-21T12:00:00"), event("2026-09-30T12:00:00"), event("2026-10-05T12:00:00")])).toEqual([
      [1, false, ["before"]], [2, false, []], [3, false, ["after"]],
    ]);
    expect(shape(rows)).toEqual([[1, false, ["before"]], [2, false, []], [3, false, ["after"]]]);
  });

  test("gaps outside the timetable coverage are not called breaks", () => {
    const rows = [lecture("2026-09-21T12:05:00", "before"), lecture("2026-10-05T12:05:00", "after")];
    expect(shape(rows, [event("2026-09-21T12:00:00")])).toEqual([
      [1, false, ["before"]], [2, false, []], [3, false, ["after"]],
    ]);
  });

  test("local Mondays and Sundays stay together across UTC boundaries and DST", () => {
    expect(shape([
      lecture("2026-10-05T00:05:00", "monday"), lecture("2026-10-11T23:55:00", "sunday"),
      lecture("2026-10-12T00:05:00", "next"), lecture("2026-09-28T00:05:00", "previous"),
    ])).toEqual([[1, false, ["previous"]], [2, false, ["monday", "sunday"]], [3, false, ["next"]]]);
  });

  test("undated recordings are kept even when no dated recording exists", () => {
    expect(shape([lecture("bad", "undated")])).toEqual([[null, false, ["undated"]]]);
    expect(shape([])).toEqual([]);
  });
});
