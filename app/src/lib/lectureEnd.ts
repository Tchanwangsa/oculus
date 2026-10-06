import { listen } from "@tauri-apps/api/event";

import { getLectureEnd, type Lecture, type LectureEndRow } from "@/lib/db";
import {
  LECTURE_END_EVENT,
  LECTURES_CHANGED_EVENT,
  findLectureEnd,
  type LectureEndFinished,
} from "@/lib/lectures";
import { fmtClockSecs } from "@/lib/media";

/**
 * Where the player treats a lecture as over (docs/chapters.md, "Where the
 * lecture ends"): Done and the Up Next card both count from `lectureEnd`.
 *
 * The `lectures` row a player holds is a snapshot the end job outlives, so
 * what SQLite and the job's event said last is kept here, keyed by lecture.
 * Module-level: playback (`lib/lecturePlayback.ts`) reads it with no player
 * mounted, and a run outlives the player that started it.
 */

/** The end columns as last read or reported; `status` as `content_end_status`. */
export interface LectureEndState {
  status: string | null;
  seconds: number | null;
  error: string | null;
}

/** Seconds before a found end that count as watched; `store::save_content_end` uses the same. */
const DONE_BEFORE_END = 10;
/** Without a found end: seconds before the file's end that count as watched. */
const COMPLETE_WITHIN = 30;
/** Without a found end: Up Next shows this long before the file ends. */
const UP_NEXT_BEFORE_FILE_END = 20;

const known = new Map<string, LectureEndState>();
/** Bumped per lecture by each event, so a read that started before it is dropped. */
const reported = new Map<string, number>();
/** Lectures this session has started a run for on open: once each, across
 *  remounts and StrictMode's double effects. */
const asked = new Set<string>();
const subscribers = new Set<() => void>();
let listening = false;

function put(id: string, state: LectureEndState) {
  known.set(id, state);
  for (const fn of subscribers) fn();
}

const fromRow = (row: LectureEndRow): LectureEndState => ({
  status: row.content_end_status,
  seconds: row.content_end_seconds,
  error: row.content_end_error,
});

function listenOnce() {
  if (listening) return;
  listening = true;
  void listen<LectureEndFinished>(LECTURE_END_EVENT, ({ payload: p }) => {
    reported.set(p.lectureId, (reported.get(p.lectureId) ?? 0) + 1);
    put(p.lectureId, { status: p.status, seconds: p.seconds, error: p.error });
    // The run may have marked the lecture Done (`store::save_content_end`).
    window.dispatchEvent(new CustomEvent(LECTURES_CHANGED_EVENT));
  });
}

export function subscribeLectureEnds(fn: () => void): () => void {
  subscribers.add(fn);
  return () => subscribers.delete(fn);
}

/** The freshest end state this session has for `id`, or undefined. */
export function lectureEndState(id: string): LectureEndState | undefined {
  return known.get(id);
}

/** The found content end in seconds, when the job found one. */
export function contentEnd(lecture: Lecture): number | null {
  const s = known.get(lecture.id) ?? {
    status: lecture.content_end_status,
    seconds: lecture.content_end_seconds,
  };
  return s.status === "ready" && s.seconds != null ? s.seconds : null;
}

export interface LectureEnd {
  seconds: number;
  /** The job's end, not the file's. */
  found: boolean;
}

/** Where the player treats `lecture` as over: the found content end, else the
 *  file's. Pass the element's duration (0 if unknown); Echo360's catalogue
 *  length, the fallback, runs short. Null with neither. */
export function lectureEnd(lecture: Lecture, fileDuration: number): LectureEnd | null {
  const found = contentEnd(lecture);
  if (found != null) return { seconds: found, found: true };
  const total = fileDuration > 0 ? fileDuration : lecture.duration_seconds;
  return total > 0 ? { seconds: total, found: false } : null;
}

/** Whether a position counts as having watched the lecture — Done. */
export function isWatched(lecture: Lecture, at: number, fileDuration: number): boolean {
  const end = lectureEnd(lecture, fileDuration);
  if (!end) return false;
  return at >= end.seconds - (end.found ? DONE_BEFORE_END : COMPLETE_WITHIN);
}

/** How far through the lecture its saved position is, 0–1, against the end
 *  Done counts to — the catalogue length stands in for the file's. */
export function watchedFraction(lecture: Lecture): number {
  const end = lectureEnd(lecture, 0);
  return end ? Math.min(1, lecture.progress_seconds / end.seconds) : 0;
}

/** The list's status text: Done, time left to the lecture's end, or unwatched.
 *  The `> 5` must match `getRecentlyWatchedLectures` (lib/db.ts). */
export function progressLabel(lecture: Lecture): { text: string; color: string } {
  if (lecture.completed) return { text: "Done", color: "text-success" };
  if (lecture.progress_seconds > 5) {
    const end = lectureEnd(lecture, 0)?.seconds ?? 0;
    const left = Math.max(0, end - lecture.progress_seconds);
    return { text: `${fmtClockSecs(left)} left`, color: "text-warning" };
  }
  return { text: "Not watched", color: "text-muted-foreground" };
}

/** The second from which Up Next shows: the found end, else just before the file's. */
export function upNextFrom(lecture: Lecture, fileDuration: number): number | null {
  const end = lectureEnd(lecture, fileDuration);
  if (!end) return null;
  return end.found ? end.seconds : Math.max(0, end.seconds - UP_NEXT_BEFORE_FILE_END);
}

/** Start a run, showing it as running at once; a refusal or a missing
 *  transcript leaves SQLite as it was, so it is read back to say which. */
function start(id: string) {
  const prior = known.get(id);
  put(id, { status: "running", seconds: prior?.seconds ?? null, error: null });
  findLectureEnd(id).catch(async (e) => {
    const row = await getLectureEnd(id).catch(() => null);
    if (row?.content_end_status) put(id, fromRow(row));
    else put(id, { status: "error", seconds: prior?.seconds ?? null, error: String(e) });
  });
}

/**
 * The player opened `id`: read its end columns from SQLite and, the first time
 * a lecture with a transcript has never been run (`NULL`), find its end. Never
 * on `running`, `ready`, `none` or `error` — an error waits for Retry.
 */
export async function openLectureEnd(id: string): Promise<void> {
  listenOnce();
  const seen = reported.get(id) ?? 0;
  const row = await getLectureEnd(id).catch(() => null);
  if (!row || (reported.get(id) ?? 0) !== seen) return;
  // A failure to start leaves the column NULL; keep showing it.
  if (!(row.content_end_status === null && known.get(id)?.status === "error")) {
    put(id, fromRow(row));
  }
  if (row.content_end_status === null && row.transcript_path && !asked.has(id)) {
    asked.add(id);
    start(id);
  }
}

/**
 * A list showed these lectures: find the end of each one started, unfinished
 * and never run, so a position already past the end marks it Done
 * (`store::save_content_end`) without reopening it. Once per lecture per
 * session, sharing `openLectureEnd`'s set.
 */
export function findStartedLectureEnds(lectures: Lecture[]): void {
  for (const l of lectures) {
    const status = known.get(l.id)?.status ?? l.content_end_status;
    if (l.completed || l.progress_seconds <= 5 || status !== null) continue;
    if (!l.transcript_path || asked.has(l.id)) continue;
    listenOnce();
    asked.add(l.id);
    start(l.id);
  }
}

/** Retry after an error. No force: `store::claim_content_end` re-runs an
 *  `error`, and refuses rather than replace an end found meanwhile. */
export function retryLectureEnd(id: string): void {
  listenOnce();
  start(id);
}
