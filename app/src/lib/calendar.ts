import { invoke } from "@tauri-apps/api/core";
import {
  getCalendarEvents,
  getAllLectures,
  getLocalEvents,
  replaceCalendarEvents,
  type CalendarEventData,
  type DbCalendarEvent,
  type DbLocalEvent,
} from "@/lib/db";
import { getAllOpenTasks } from "@/lib/projects";
import { displayCode, sameDay, sqliteUtcToMs, startOfDay } from "@/lib/format";

/**
 * `class`/`due` come from Canvas (or `local_events`, drawn the same way);
 * `lecture` from Echo360 recordings; `note` is a user reminder; `task` is a
 * dated open project task, read live. See `docs/calendar.md`.
 */
export type CalKind = "class" | "due" | "lecture" | "note" | "task";

/** Subject id for rows with no subject; negative, so it never collides with a
 *  Canvas course id. */
export const NO_SUBJECT = -1;

export const NO_SUBJECT_LABEL = "Personal";

/** Fired after a sync refreshes the calendar rows. */
export const CALENDAR_UPDATED_EVENT = "oculus:calendar-updated";

/** Re-fetches one subject's Canvas calendar and replaces its stored rows. */
export async function syncCalendar(canvasCourseId: number): Promise<void> {
  const rows = await invoke<CalendarEventData[]>("calendar_sync_events", { canvasCourseId });
  await replaceCalendarEvents(canvasCourseId, rows);
}

export interface CalEvent {
  id: string;
  kind: CalKind;
  subjectId: number;
  /** Bare course code for display: "MULT20015", not "MULT20015_2026_SM2". */
  subjectCode: string;
  title: string;
  start: Date;
  /** `null` for deadlines, which are an instant rather than a span. */
  end: Date | null;
  allDay: boolean;
  location: string | null;
  url: string | null;
  description: string | null;
  /** Set on `lecture` rows so the event can open the player. */
  lectureId: string | null;
  /** `local_events.id`, the handle the calendar deletes by. `null` for synced
   *  rows, which a sync would only bring back. */
  localId: number | null;
  /** `local_events.source`, shown on the card. */
  localSource: string | null;
  /** `project_tasks.id` for a `task` row; `null` on every other layer. */
  taskId: number | null;
  /** A `task`'s project, so its card can open the board; `null` elsewhere and
   *  on an unfiled task. */
  projectId: number | null;
  projectName: string | null;
}

/** An instant rather than a span: drawn as a marker over the week grid, not a
 *  block competing with the classes. */
export function isInstant(e: CalEvent): boolean {
  return e.kind === "due" || e.kind === "note" || e.kind === "task";
}

/** An instant the user set themselves, drawn quieter than a course deadline. */
export function isSelfImposed(e: CalEvent): boolean {
  return e.kind === "note" || e.kind === "task";
}

// ── Loading ───────────────────────────────────────────────────────────────────

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

// ── Subject colour ────────────────────────────────────────────────────────────

/** Chart tokens, redefined for dark mode in `index.css`. */
const PALETTE = [
  "var(--color-chart-1)",
  "var(--color-chart-2)",
  "var(--color-chart-3)",
  "var(--color-chart-4)",
  "var(--color-chart-5)",
];

/**
 * A stable colour per subject, indexed by sorted id. {@link NO_SUBJECT} stays
 * out of the indexing (and takes `primary`), or the first personal note would
 * recolour every subject.
 */
export function subjectColors(events: CalEvent[]): Map<number, string> {
  const ids = [...new Set(events.map((e) => e.subjectId))]
    .filter((id) => id !== NO_SUBJECT)
    .sort((a, b) => a - b);
  const colors = new Map(ids.map((id, i) => [id, PALETTE[i % PALETTE.length]]));
  colors.set(NO_SUBJECT, "var(--color-primary)");
  return colors;
}

// ── Date arithmetic ───────────────────────────────────────────────────────────

export const DAY_MS = 86_400_000;

export function addDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

export function addMonths(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth() + n, 1);
}

/** Monday-first, the Australian week. */
export function startOfWeek(d: Date): Date {
  const day = (d.getDay() + 6) % 7;
  return addDays(startOfDay(d), -day);
}

/**
 * The six Monday-first weeks a month grid draws. Always six rows, so the grid
 * never changes height as you page through months.
 */
export function monthGrid(month: Date): Date[][] {
  const first = startOfWeek(new Date(month.getFullYear(), month.getMonth(), 1));
  return Array.from({ length: 6 }, (_, w) =>
    Array.from({ length: 7 }, (_, d) => addDays(first, w * 7 + d)),
  );
}

export function weekDays(anchor: Date): Date[] {
  const first = startOfWeek(anchor);
  return Array.from({ length: 7 }, (_, i) => addDays(first, i));
}

// ── Querying a loaded set ─────────────────────────────────────────────────────

export function eventsOn(events: CalEvent[], day: Date): CalEvent[] {
  return events.filter((e) => sameDay(e.start, day));
}

/** Its end (or instant) is behind `now` — what the calendar greys out. */
export function isPast(e: CalEvent, now: Date): boolean {
  return (e.end ?? e.start).getTime() < now.getTime();
}

/** Minutes from midnight — the y coordinate for a week-view block. */
export function minutesFromMidnight(d: Date): number {
  return d.getHours() * 60 + d.getMinutes();
}

/** A span with no `end_at` gets an hour, an instant half that, so it can still
 *  be drawn. */
export function durationMinutes(e: CalEvent): number {
  if (e.end == null) return isInstant(e) ? 30 : 60;
  return Math.max(15, Math.round((e.end.getTime() - e.start.getTime()) / 60_000));
}

/** Hour an event ends, from its own midnight; an end on a later day is 24, or
 *  a clock-time end past midnight would shrink the grid. */
function endHour(e: CalEvent): number {
  if (e.end == null) return e.start.getHours() + 1;
  if (startOfDay(e.end).getTime() > startOfDay(e.start).getTime()) return 24;
  return Math.ceil(minutesFromMidnight(e.end) / 60);
}

/**
 * The hour range a week grid covers: the events in view padded by an hour, at
 * least 8am–6pm. `includeHour` keeps a given hour (today's "now" line) on it.
 */
export function hourRange(
  events: CalEvent[],
  includeHour?: number,
): [number, number] {
  let lo = 8;
  let hi = 18;
  for (const e of events) {
    if (e.allDay) continue;
    lo = Math.min(lo, e.start.getHours());
    hi = Math.max(hi, endHour(e));
  }
  if (includeHour != null) {
    lo = Math.min(lo, includeHour);
    hi = Math.max(hi, includeHour + 1);
  }
  return [Math.max(0, lo - 1), Math.min(24, hi + 1)];
}

/** "PAR-148B-B1-B101-Kathleen Fitzpatrick Theatre" → "B101 · Kathleen
 *  Fitzpatrick Theatre": the room code leads, since a truncated prefix locates
 *  nothing. */
export function shortLocation(loc: string): string {
  const parts = loc.split("-");
  if (parts.length < 5) return loc;
  const codes = parts.slice(0, 4);
  if (!codes.every((c) => c.length > 0 && c.length <= 5 && /^[A-Za-z0-9]+$/.test(c))) {
    return loc;
  }
  return `${codes[3]} · ${parts.slice(4).join("-")}`;
}

export function fmtEventTime(e: CalEvent): string {
  if (e.allDay) return "All day";
  const t = (d: Date) =>
    d.toLocaleTimeString("en-AU", { hour: "numeric", minute: "2-digit" }).replace(" ", "");
  return e.end && !isInstant(e) ? `${t(e.start)}–${t(e.end)}` : t(e.start);
}

export function fmtMonth(d: Date): string {
  return d.toLocaleDateString("en-AU", { month: "long", year: "numeric" });
}

/** "11 – 17 Aug 2026", collapsing the month or year when both ends share it. */
export function fmtWeekRange(anchor: Date): string {
  const days = weekDays(anchor);
  const a = days[0];
  const b = days[6];
  const sameMonth = a.getMonth() === b.getMonth() && a.getFullYear() === b.getFullYear();
  const left = a.toLocaleDateString("en-AU", {
    day: "numeric",
    ...(sameMonth ? {} : { month: "short" }),
  });
  const right = b.toLocaleDateString("en-AU", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
  return `${left} – ${right}`;
}
