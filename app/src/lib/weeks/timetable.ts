import type { DbCalendarEvent } from "@/lib/db";
import { weekKey } from "./dates";

/** Fraction of timetabled subjects needed to call a term week teaching. */
export const TEACHING_WEEK_THRESHOLD = 0.5;

export function timedClasses(classes: DbCalendarEvent[]): DbCalendarEvent[] {
  return classes.filter((event) => event.kind === "class" && !event.all_day
    && Number.isFinite(Date.parse(event.start_at)));
}

export function termTimetable(classes: DbCalendarEvent[]) {
  const bySubject = new Map<number, DbCalendarEvent[]>();
  for (const event of timedClasses(classes)) {
    const rows = bySubject.get(event.subject_id) ?? [];
    rows.push(event);
    bySubject.set(event.subject_id, rows);
  }

  const subjectWeeks = new Map<number, Set<number>>();
  const subjectsPerWeek = new Map<number, number>();
  for (const [id, rows] of bySubject) {
    // Prefer named lectures per subject so tutorials do not fill lecture gaps.
    const lectures = rows.filter((event) => /\blectures?\b/i.test(event.title));
    const preferred = lectures.length > 0 ? lectures : rows;
    const weeks = new Set(preferred.map((event) => weekKey(new Date(event.start_at))));
    subjectWeeks.set(id, weeks);
    for (const key of weeks) subjectsPerWeek.set(key, (subjectsPerWeek.get(key) ?? 0) + 1);
  }

  const minimum = Math.ceil(bySubject.size * TEACHING_WEEK_THRESHOLD);
  const teaching = new Set([...subjectsPerWeek]
    .filter(([, count]) => count >= minimum)
    .map(([key]) => key)
    .sort((a, b) => a - b));
  return { subjectWeeks, teaching };
}
