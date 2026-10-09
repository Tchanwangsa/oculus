import { invoke } from "@tauri-apps/api/core";
import { replaceCalendarEvents, type CalendarEventData } from "@/lib/db";

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
