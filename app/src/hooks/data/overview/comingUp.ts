import type { DbCalendarEvent } from "@/lib/db";
import type { DbOpenTask } from "@/lib/planning/projects";
import { taskHref } from "@/components/projects/nav/taskHref";
import { filePagePath } from "@/lib/files/labels";
import type { TaskDoc } from "@/pages/subject/content/assignments/grouping";
import { localRowDate, sourceDate } from "./dates";
import type { DeadlineItem } from "./types";

export function deriveComingUp(subjectId: number, calendar: DbCalendarEvent[], tasks: DbOpenTask[], docs: TaskDoc[], now: Date): DeadlineItem[] {
  const out: DeadlineItem[] = [];
  for (const row of calendar) {
    if (row.subject_id !== subjectId || row.kind !== "due") continue;
    const date = sourceDate(row.start_at);
    if (!date || date < now) continue;
    // Calendar assignment ids and source URLs survive a title or due-date edit.
    const ownDocs = docs.filter(({ file }) => file.subject_id === subjectId);
    const direct = ownDocs.find(({ file, sourceUrl }) =>
      (file.category === "assignment" && file.canvas_id != null && row.id === `assignment_${file.canvas_id}`)
        || (sourceUrl != null && row.url === sourceUrl)
        || (sourceUrl != null && /\/assignments\/(\d+)(?:[?#]|$)/.exec(sourceUrl)?.[1] === row.id.replace(/^assignment_/, "")));
    // Quiz files carry the quiz id while calendar rows carry its assignment shell.
    // Match only an unambiguous title and due minute when no stable id is available.
    const candidates = ownDocs.filter((doc) => doc.title === row.title && doc.due != null
      && Math.floor(doc.due.getTime() / 60_000) === Math.floor(date.getTime() / 60_000));
    const doc = direct ?? (candidates.length === 1 ? candidates[0] : undefined);
    if (doc?.submitted) continue;
    out.push({
      id: `due:${row.id}`, kind: "due", title: row.title, date,
      href: doc ? filePagePath(subjectId, doc.file.relative_path) : row.url,
    });
  }
  for (const task of tasks) {
    if (task.project_subject_id !== subjectId || task.done_at != null) continue;
    const date = localRowDate(task.due_at);
    if (!date || date < now) continue;
    out.push({ id: `task:${task.id}`, kind: "task", title: task.title, date, href: taskHref(task.project_id, task) });
  }
  return out.sort((a, b) => a.date.getTime() - b.date.getTime() || a.id.localeCompare(b.id)).slice(0, 5);
}
