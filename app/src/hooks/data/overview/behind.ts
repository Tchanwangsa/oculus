import type { DbCalendarEvent, Lecture } from "@/lib/db";
import { isWatched } from "@/lib/lectures/end";
import { recordingStatus, weekOf, type SubjectWeeks } from "@/lib/weeks";

export function watched(lecture: Lecture): boolean {
  return !!lecture.completed || isWatched(lecture, lecture.progress_seconds, 0);
}

export function deriveBehind(subjectId: number, strip: SubjectWeeks, lectures: Lecture[], classes: DbCalendarEvent[], now: Date): { count: number; lectures: Lecture[] } {
  const current = strip.weeks[strip.current];
  if (current?.kind !== "teaching" || now < current.start) return { count: 0, lectures: [] };
  const keys = new Set(strip.weeks.slice(0, strip.current + 1).filter((week) => week.kind === "teaching").map((week) => week.key));
  const behind = lectures.filter((lecture) => {
    const date = new Date(lecture.date);
    const key = weekOf(strip, date);
    return lecture.subject_id === subjectId && date <= now && key != null && keys.has(key)
      && recordingStatus(lecture, classes) !== "untimetabled" && !watched(lecture);
  }).sort((a, b) => Date.parse(a.date) - Date.parse(b.date) || a.id.localeCompare(b.id));
  return { count: behind.length, lectures: behind };
}
