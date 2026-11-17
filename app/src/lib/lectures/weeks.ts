import type { Lecture } from "@/lib/db";
import { displayLocale } from "@/lib/format/locale";
import { addDays } from "@/lib/planning/calendar/dates";
import { weekOf, type SubjectWeeks } from "@/lib/weeks";

export interface LectureWeek {
  key: number;
  label: string;
  isBreak: boolean;
  lectures: Lecture[];
  start: string;
  end: string;
}

function shortDate(date: Date): string {
  return date.toLocaleDateString(displayLocale, { day: "numeric", month: "short" });
}

/** Present recordings in the shared strip, keeping undated rows playable. */
export function groupLecturesByWeek(
  lectures: Lecture[],
  strip: SubjectWeeks,
): LectureWeek[] {
  const valid = lectures
    .map((lecture) => ({ lecture, date: new Date(lecture.date) }))
    .filter(({ date }) => Number.isFinite(date.getTime()))
    .sort((a, b) => a.date.getTime() - b.date.getTime());
  const undated = lectures.filter((lecture) => !Number.isFinite(Date.parse(lecture.date)));
  const rows = new Map<number, Lecture[]>();
  for (const { lecture, date } of valid) {
    const key = weekOf(strip, date);
    if (key === null) continue;
    const group = rows.get(key) ?? [];
    group.push(lecture);
    rows.set(key, group);
  }
  const result: LectureWeek[] = strip.weeks.map((week) => ({
    key: week.key,
    label: week.kind === "teaching" ? `Week ${week.label}` : week.label,
    isBreak: week.kind === "break",
    lectures: rows.get(week.key) ?? [],
    start: shortDate(week.start),
    end: shortDate(addDays(week.end, -1)),
  }));
  if (strip.weeks.length === 0 && valid.length > 0) {
    result.push({ key: -2, label: "Recordings", isBreak: false, lectures: valid.map(({ lecture }) => lecture), start: "", end: "" });
  }
  if (undated.length > 0) {
    result.push({ key: -1, label: "Undated", isBreak: false, lectures: undated, start: "", end: "" });
  }
  return result;
}
