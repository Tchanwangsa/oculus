import { describe, expect, test } from "bun:test";
import { groupLecturesByWeek } from "@/lib/lectures/weeks";
import { subjectWeeks, type SubjectWeeksInput } from "@/lib/weeks";
import { event, lecture, subject } from "../weeks/fixtures";

const groups = (extra: Partial<SubjectWeeksInput> = {}) => {
  const input = {
    subjectId: 1, subjects: [subject(1), subject(2)], classes: [], lectures: [],
    now: new Date("2026-09-21T12:00:00"), ...extra,
  };
  return groupLecturesByWeek(input.lectures, subjectWeeks(input));
};
const shape = (extra: Partial<SubjectWeeksInput>) => groups(extra)
  .map((group) => [group.label, group.isBreak, group.lectures.map((row) => row.id)]);

describe("lecture groups use the shared teaching strip", () => {
  test("term numbering starts before the subject's first recording", () => {
    expect(shape({ classes: [event("2026-09-14T11:00:00", 2), event("2026-09-21T11:00:00")],
      lectures: [lecture("2026-09-21T11:05:00", 1, "recording")],
    })).toEqual([
      ["Week 1", false, []], ["Week 2", false, ["recording"]],
      ["+1", false, []], ["+2", false, []], ["+3", false, []],
    ]);
  });

  test("recordings inside breaks stay playable and teaching numbers resume", () => {
    expect(shape({ classes: [event("2026-09-21T11:00:00"), event("2026-10-05T11:00:00")],
      lectures: [lecture("2026-09-30T11:05:00", 1, "break")],
    })).toEqual([
      ["Week 1", false, []], ["Break", true, ["break"]], ["Week 2", false, []],
      ["+1", false, []], ["+2", false, []], ["+3", false, []],
    ]);
  });

  test("dates outside the strip fold into its edges without extending it", () => {
    const result = groups({ classes: [event("2026-09-21T11:00:00")], lectures: [
      lecture("2028-01-01T11:05:00", 1, "late"), lecture("2026-08-01T11:05:00", 1, "early"),
      lecture("2026-09-21T11:05:00", 1, "class"),
    ] });
    expect(result).toHaveLength(4);
    expect(result[0].lectures.map((row) => row.id)).toEqual(["early", "class"]);
    expect(result[3].lectures.map((row) => row.id)).toEqual(["late"]);
  });

  test("recording fallback fills empty weeks and keeps undated recordings", () => {
    expect(shape({ lectures: [lecture("2026-09-21T11:00:00", 1, "first"), lecture("2026-10-05T11:00:00", 1, "last"), lecture("bad", 1, "undated")] })).toEqual([
      ["Week 1", false, ["first"]], ["Week 2", false, []], ["Week 3", false, ["last"]],
      ["+1", false, []], ["+2", false, []], ["+3", false, []], ["Undated", false, ["undated"]],
    ]);
  });

  test("empty and undated-only catalogues need no fabricated week", () => {
    expect(groups()).toEqual([]);
    expect(shape({ lectures: [lecture("bad", 1, "undated")] })).toEqual([["Undated", false, ["undated"]]]);
  });

  test("recordings remain visible when the term has no qualifying week", () => {
    expect(shape({ subjects: [subject(1), subject(2), subject(3)], classes: [
      event("2026-09-07T11:00:00", 1), event("2026-09-14T11:00:00", 2), event("2026-09-21T11:00:00", 3),
    ], lectures: [lecture("2026-09-07T11:05:00", 1, "recording")] })).toEqual([["Recordings", false, ["recording"]]]);
  });
});
