import type { Lecture } from "@/lib/db";
import { weekOf, type SubjectWeeks } from "@/lib/weeks";
import { watched } from "./behind";
import type { WeekProgress } from "./types";

export function deriveProgress(subjectId: number, strip: SubjectWeeks, lectures: Lecture[]): WeekProgress[] {
  const progress = new Map(strip.weeks.filter((week) => week.kind === "teaching")
    .map((week) => [week.key, { weekKey: week.key, watched: 0, total: 0 }]));
  for (const lecture of lectures) {
    if (lecture.subject_id !== subjectId) continue;
    const key = weekOf(strip, new Date(lecture.date));
    const week = key == null ? undefined : progress.get(key);
    if (!week) continue;
    week.total++;
    if (watched(lecture)) week.watched++;
  }
  return [...progress.values()];
}
