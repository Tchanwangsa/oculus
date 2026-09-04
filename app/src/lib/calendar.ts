import {
  getCalendarEvents,
  getAllLectures,
  getLocalEvents,
  type DbCalendarEvent,
  type DbLocalEvent,
} from "@/lib/db";
import { displayCode } from "@/lib/format";

/**
 * What the calendar can draw.
 *
 * `class` and `due` come from Canvas's calendar API (see
 * `app/src-tauri/src/calendar.rs`). `lecture` is derived from the Echo360
 * recordings already in the library — a lecture recording *is* a class that
 * happened, so it fills the timetable in for any subject whose staff never
 * published events to the Canvas calendar. `note` is Oculus's own: a reminder
 * an automation or the user wrote, which has no Canvas counterpart at all.
 *
 * A local event can also be a `class` or a `due` — it is the same kind of
 * thing, just stored elsewhere (`local_events`, see `docs/calendar.md`), so it
 * is drawn the same way rather than as a fourth layer.
 */
export type CalKind = "class" | "due" | "lecture" | "note";

/**
 * The subject id a local event with no subject is filed under.
 *
 * Everything downstream — the colour map, the header's subject filter — keys
 * off a subject id, so a personal reminder needs one. Negative, because Canvas
 * course ids are positive and always will be.
 */
export const NO_SUBJECT = -1;

/** What the filter chip and the detail card call {@link NO_SUBJECT}. */
export const NO_SUBJECT_LABEL = "Personal";

/** Fired after a sync refreshes the calendar rows, so an open calendar page
 *  re-reads them without polling. */
export const CALENDAR_UPDATED_EVENT = "oculus:calendar-updated";

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
  /** `local_events.id` for a row Oculus wrote itself — the handle the calendar
   *  deletes by. `null` for anything Canvas or Echo360 owns, which is exactly
   *  the set the user must *not* be able to delete: a sync would bring it
   *  straight back. */
  localId: number | null;
  /** `automation` or `manual` — why a local event exists, shown on its card so
   *  a deadline the user never typed is explicable. */
  localSource: string | null;
}

/**
 * An instant rather than a span: a deadline, or a note pinned to a time.
 *
 * The distinction the week grid runs on — an instant is drawn as a marker laid
 * over the hours, never as a block competing with the classes, and it is kept
 * out of the fitted hour range so an 11:59pm cutoff cannot pin every week open
 * to midnight.
 */
export function isInstant(e: CalEvent): boolean {
  return e.kind === "due" || e.kind === "note";
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
  };
}

/** A row of `local_events` — Oculus's own, and the only kind the calendar can
 *  delete. Its `notes` become the card's body, where a Canvas event has its
 *  description. */
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
  };
}

/**
 * Every dated item across every subject, in one list.
 *
 * The whole set is loaded at once — a semester across a handful of subjects is
 * a few hundred rows — so moving between months is pure arithmetic and never
 * another query.
 */
export async function loadCalendar(): Promise<CalEvent[]> {
  const [rows, lectures, local] = await Promise.all([
    getCalendarEvents(),
    getAllLectures(),
    getLocalEvents(),
  ]);

  const out: CalEvent[] = [];
  for (const r of rows) {
    const e = fromDbRow(r);
    if (e) out.push(e);
  }

  // A subject whose Canvas calendar carries class events already shows its
  // timetable; adding the Echo360 recording of the same lecture would draw
  // every class twice. Recordings therefore fill in only where Canvas is
  // silent — which is most of the time, since publishing class times to the
  // Canvas calendar is up to each subject's staff.
  const timetabled = new Set(out.filter((e) => e.kind === "class").map((e) => e.subjectId));

  for (const l of lectures) {
    if (timetabled.has(l.subject_id)) continue;
    // Echo360 stamps its start times in local wall clock with no zone marker,
    // which `new Date` reads as local — the right reading for a room booking.
    const start = new Date(l.date);
    if (Number.isNaN(start.getTime())) continue;
    out.push({
      id: `lecture_${l.id}`,
      kind: "lecture",
      subjectId: l.subject_id,
      subjectCode: displayCode(l.subject_code),
      // The Echo360 title repeats the course code and section ("COMP30026_2026_SM2
      // WE B101"); the calendar already shows the subject, so keep the tail.
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
    });
  }

  // Merged last, and deliberately after `timetabled` was taken: a class Oculus
  // wrote itself says nothing about whether Canvas published a timetable, so
  // it must not suppress a subject's recordings.
  for (const r of local) {
    const e = fromLocalRow(r);
    if (e) out.push(e);
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

/** The chart tokens, which are redefined for dark mode in `index.css` — so a
 *  colour picked here stays legible in both themes. */
const PALETTE = [
  "var(--color-chart-1)",
  "var(--color-chart-2)",
  "var(--color-chart-3)",
  "var(--color-chart-4)",
  "var(--color-chart-5)",
];

/**
 * A stable colour per subject: the palette indexed by the subject's position in
 * the sorted id list, so a subject keeps its colour as long as the set does —
 * and colours never shift about as events are filtered.
 *
 * Subject-less local events are kept out of that indexing and painted in the
 * brand colour instead. They are Oculus's own rather than a course's, and
 * letting them take a palette slot would recolour every subject the moment you
 * pinned your first note.
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

export function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

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

export function sameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
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

/** Over and done with: an event's end, or its instant when it has no length,
 *  is behind us. What the calendar greys out. */
export function isPast(e: CalEvent, now: Date): boolean {
  return (e.end ?? e.start).getTime() < now.getTime();
}

/** Minutes from midnight — the y coordinate for a week-view block. */
export function minutesFromMidnight(d: Date): number {
  return d.getHours() * 60 + d.getMinutes();
}

/** An instant has no length; a class with no `end_at` gets an hour so it can
 *  still be drawn as a block. */
export function durationMinutes(e: CalEvent): number {
  if (e.end == null) return isInstant(e) ? 30 : 60;
  return Math.max(15, Math.round((e.end.getTime() - e.start.getTime()) / 60_000));
}

/**
 * The hour an event finishes, as a number of hours from its own midnight.
 *
 * An event running past midnight has an end whose clock time is *smaller* than
 * its start's — 11pm to 12:30am reads as 0.5, which would shrink the grid
 * instead of growing it and leave the block drawn below the last row. Anything
 * ending on a later day is therefore 24: the end of its own day.
 */
function endHour(e: CalEvent): number {
  if (e.end == null) return e.start.getHours() + 1;
  if (startOfDay(e.end).getTime() > startOfDay(e.start).getTime()) return 24;
  return Math.ceil(minutesFromMidnight(e.end) / 60);
}

/**
 * The hour range a week grid needs to cover: the classes in view, padded by an
 * hour either side and never narrower than 8am–6pm — so a quiet week still
 * looks like a timetable rather than two rows.
 *
 * `includeHour` stretches the range to keep a given hour on the grid. The week
 * showing today passes the current hour, because a "now" line the range has
 * cropped off is worse than no line at all — at 10pm every class is over and
 * the grid would stop at 7pm with nothing marking where you are.
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

/**
 * UniMelb venues read "PAR-148B-B1-B101-Kathleen Fitzpatrick Theatre & Event
 * Venue" — campus, building, level, room, name. Truncated on a chip that
 * leaves you with "PAR-148B-B1-B1…", which locates nothing, so compact display
 * leads with the room code and drops the rest of the prefix. The full string
 * stays on the detail card.
 */
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
