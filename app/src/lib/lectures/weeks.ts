import type { DbCalendarEvent, Lecture } from "@/lib/db";
import { displayLocale } from "@/lib/format/locale";
import { addDays, startOfWeek } from "@/lib/planning/calendar/dates";

export interface LectureWeek {
  key: number;
  week: number | null;
  isBreak: boolean;
  lectures: Lecture[];
  start: string;
  end: string;
}

// Civil dates, rather than elapsed hours, keep weeks stable across DST.
function weekKey(date: Date): number {
  const monday = startOfWeek(date);
  return Date.UTC(monday.getFullYear(), monday.getMonth(), monday.getDate());
}

function shortDate(date: Date): string {
  return date.toLocaleDateString(displayLocale, { day: "numeric", month: "short" });
}

/** Calendar gaps inside a published timetable are breaks, even if Echo360
 *  captured a recording there. Missing recordings alone do not prove a break. */
export function groupLecturesByWeek(
  lectures: Lecture[],
  calendar: DbCalendarEvent[] = [],
): LectureWeek[] {
  const valid = lectures
    .map((lecture) => ({ lecture, date: new Date(lecture.date) }))
    .filter(({ date }) => Number.isFinite(date.getTime()))
    .sort((a, b) => a.date.getTime() - b.date.getTime());
  const undated = lectures.filter((lecture) => !Number.isFinite(Date.parse(lecture.date)));
  const result: LectureWeek[] = [];
  if (valid.length > 0) {
    const subjectId = valid[0].lecture.subject_id;
    const classes = calendar.filter((event) =>
      event.subject_id === subjectId && event.kind === "class" && !event.all_day
      && Number.isFinite(Date.parse(event.start_at)),
    );
    // When the calendar names lectures, tutorials and other class types cannot
    // turn a lecture-free week into a teaching week.
    const namedLectures = classes.filter((event) => /\blectures?\b/i.test(event.title));
    const timetable = namedLectures.length > 0 ? namedLectures : classes;
    const teachingWeeks = new Set(timetable.map((event) => weekKey(new Date(event.start_at))));
    const firstScheduled = teachingWeeks.size > 0 ? Math.min(...teachingWeeks) : Infinity;
    const lastScheduled = teachingWeeks.size > 0 ? Math.max(...teachingWeeks) : -Infinity;
    const rows = new Map<number, Lecture[]>();
    for (const { lecture, date } of valid) {
      const key = weekKey(date);
      const group = rows.get(key) ?? [];
      group.push(lecture);
      rows.set(key, group);
    }

    const first = startOfWeek(valid[0].date);
    const last = weekKey(valid[valid.length - 1].date);
    let week = 0;
    for (let monday = first; weekKey(monday) <= last; monday = addDays(monday, 7)) {
      const key = weekKey(monday);
      const isBreak = key > firstScheduled && key < lastScheduled && !teachingWeeks.has(key);
      if (!isBreak) week++;
      const group = rows.get(key) ?? [];
      result.push({
        key,
        week: isBreak ? null : week,
        isBreak,
        lectures: group,
        start: shortDate(isBreak || group.length === 0 ? monday : new Date(group[0].date)),
        end: shortDate(isBreak || group.length === 0 ? addDays(monday, 6) : new Date(group[group.length - 1].date)),
      });
    }
  }
  if (undated.length > 0) {
    result.push({ key: -1, week: null, isBreak: false, lectures: undated, start: "", end: "" });
  }
  return result;
}
