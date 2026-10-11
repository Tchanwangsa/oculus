import {
  getCalendarEvents,
  getAllLectures,
  getLocalEvents,
  type DbCalendarEvent,
  type DbLocalEvent,
} from "@/lib/db";
import { getAllOpenTasks } from "@/lib/planning/projects";
import { displayCode, sqliteUtcToMs } from "@/lib/format/format";
import { NO_SUBJECT, NO_SUBJECT_LABEL, type CalEvent } from "./model";

function fromDbRow(r: DbCalendarEvent): CalEvent | null {
  const start = new Date(r.start_at);
  if (Number.isNaN(start.getTime())) return null;
  const end = r.end_at ? new Date(r.end_at) : null;
  return {
    id: r.id,
    kind: r.kind === "due" ? "due" : "class",
    subjectId: r.subject_id,
    subjectCode: displayCode(r.subject_code),
    title: r.title,
    start,
    end: end && !Number.isNaN(end.getTime()) ? end : null,
    allDay: r.all_day === 1,
    location: r.location,
    url: r.url,
    description: r.description,
    lectureId: null,
    localId: null,
    localSource: null,
    taskId: null,
    projectId: null,
    projectName: null,
  };
}

function fromLocalRow(r: DbLocalEvent): CalEvent | null {
  const start = new Date(r.start_at);
  if (Number.isNaN(start.getTime())) return null;
  const end = r.end_at ? new Date(r.end_at) : null;
  return {
    id: `local_${r.id}`,
    kind: r.kind === "due" ? "due" : r.kind === "class" ? "class" : "note",
    subjectId: r.subject_id ?? NO_SUBJECT,
    subjectCode: r.subject_code ? displayCode(r.subject_code) : NO_SUBJECT_LABEL,
    title: r.title,
    start,
    end: end && !Number.isNaN(end.getTime()) ? end : null,
    allDay: r.all_day === 1,
    location: null,
    url: null,
    description: r.notes,
    lectureId: null,
    localId: r.id,
    localSource: r.source,
    taskId: null,
    projectId: null,
    projectName: null,
  };
}

/** Every dated item across every subject, loaded at once so paging months is
 *  pure arithmetic. */
export async function loadCalendar(): Promise<CalEvent[]> {
  const [rows, lectures, local, tasks] = await Promise.all([
    getCalendarEvents(),
    getAllLectures(),
    getLocalEvents(),
    getAllOpenTasks(),
  ]);

  const out: CalEvent[] = [];
  for (const r of rows) {
    const e = fromDbRow(r);
    if (e) out.push(e);
  }

  // Recordings fill in only for subjects whose Canvas calendar has no classes,
  // or every class would be drawn twice.
  const timetabled = new Set(out.filter((e) => e.kind === "class").map((e) => e.subjectId));

  for (const l of lectures) {
    if (timetabled.has(l.subject_id)) continue;
    // Echo360 stamps local wall-clock time with no zone, which `new Date` reads
    // as local — correct here.
    const start = new Date(l.date);
    if (Number.isNaN(start.getTime())) continue;
    out.push({
      id: `lecture_${l.id}`,
      kind: "lecture",
      subjectId: l.subject_id,
      subjectCode: displayCode(l.subject_code),
      title: lectureLabel(l.title, l.subject_code),
      start,
      end:
        l.duration_seconds > 0
          ? new Date(start.getTime() + l.duration_seconds * 1000)
          : null,
      allDay: false,
      location: null,
      url: null,
      description: null,
      lectureId: l.id,
      localId: null,
      localSource: null,
      taskId: null,
      projectId: null,
      projectName: null,
    });
  }

  // After `timetabled` on purpose: a local class must not suppress recordings.
  for (const r of local) {
    const e = fromLocalRow(r);
    if (e) out.push(e);
  }

  // Tasks are read live, never copied into `local_events`, so nothing goes
  // stale. `getAllOpenTasks` already drops undated and finished ones.
  for (const t of tasks) {
    // `due_at` is ISO8601 from the UI or SQLite's "YYYY-MM-DD HH:MM:SS" from the
    // CLI; `sqliteUtcToMs` reads both.
    const ms = sqliteUtcToMs(t.due_at);
    if (ms == null) continue;
    out.push({
      id: `task_${t.id}`,
      kind: "task",
      // The subject is the project's; personal and unfiled tasks get NO_SUBJECT.
      subjectId: t.project_subject_id ?? NO_SUBJECT,
      subjectCode: t.project_subject_code
        ? displayCode(t.project_subject_code)
        : NO_SUBJECT_LABEL,
      title: t.title,
      start: new Date(ms),
      end: null,
      allDay: false,
      location: null,
      url: null,
      description: t.body,
      lectureId: null,
      localId: null,
      localSource: null,
      taskId: t.id,
      projectId: t.project_id,
      projectName: t.project_name,
    });
  }

  out.sort((a, b) => a.start.getTime() - b.start.getTime());
  return out;
}

/** "COMP30026_2026_SM2 WE B101" → "WE B101"; anything else is left alone. */
export function lectureLabel(title: string, subjectCode: string): string {
  const stripped = title.replace(subjectCode, "").trim();
  return stripped.length > 0 ? stripped : title;
}
