import type { DbCalendarEvent, Lecture } from "@/lib/db";
import { lecturePagePath } from "@/lib/lectures/pagePath";
import type { SubjectWeeks } from "@/lib/weeks";
import { inWeek, sourceDate } from "./dates";
import type { EventItem } from "./types";

export function deriveEvents(subjectId: number, strip: SubjectWeeks, key: number | null, classes: DbCalendarEvent[], lectures: Lecture[], now: Date): EventItem[] {
  const out: EventItem[] = [];
  const own = classes.filter((row) => row.subject_id === subjectId && row.kind === "class" && sourceDate(row.start_at));
  for (const row of own) {
    const date = sourceDate(row.start_at);
    if (!inWeek(strip, key, date)) continue;
    const end = sourceDate(row.end_at);
    out.push({
      id: `class:${row.id}`, kind: "class", title: row.title, date, end,
      allDay: row.all_day === 1, location: row.location, href: row.url, past: (end ?? date) < now,
    });
  }
  // Use the entire subject's Canvas layer to decide fallback, not just this week.
  if (own.length === 0) for (const lecture of lectures) {
    if (lecture.subject_id !== subjectId) continue;
    const date = sourceDate(lecture.date);
    if (!inWeek(strip, key, date)) continue;
    const end = lecture.duration_seconds > 0 ? new Date(date.getTime() + lecture.duration_seconds * 1000) : null;
    out.push({
      id: `recording:${lecture.id}`, kind: "recording", title: lecture.title, date, end,
      allDay: false, location: null, href: lecturePagePath(lecture), past: (end ?? date) < now,
    });
  }
  return out.sort((a, b) => a.date.getTime() - b.date.getTime() || a.id.localeCompare(b.id));
}
