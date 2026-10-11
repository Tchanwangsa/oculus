import { getDb } from "./connection";

/** A capture's stream: 1 the presenter screen, 2 the room camera. */
export type SourceNum = 1 | 2;

export interface Lecture {
  id: string;
  lesson_id: string;
  subject_id: number;
  title: string;
  date: string;
  duration_seconds: number;
  video_path: string | null;
  /** The camera stream, downloaded separately and often not at all. */
  video2_path: string | null;
  /** 1 when Echo360 publishes a camera stream for this capture. */
  has_source2: number;
  transcript_path: string | null;
  progress_seconds: number;
  /** UTC `datetime('now')`, comparable with `files.last_accessed_at`; NULL
   *  is never watched. Home's Recent row ranks on it. */
  last_watched_at: string | null;
  completed: number;
  synced_at: string;
  /** `null` (never run) | `running` | `ready` | `error`. */
  chapter_status: string | null;
  /** Stamped only by a terminal status. */
  chaptered_at: string | null;
  /** Why the last run failed; cleared on success. */
  chapter_error: string | null;
  /** Where the lecture's planned content ends (`lecture_end`): the end of the
   *  sign-off line, in seconds. Kept when a later run fails. */
  content_end_seconds: number | null;
  /** The words of the sign-off the end was found on. */
  content_end_quote: string | null;
  /** `null` (never run) | `running` | `ready` | `none` (cut off, no end) | `error`. */
  content_end_status: string | null;
  content_end_error: string | null;
}

export interface LectureData {
  id: string;
  lesson_id: string;
  title: string;
  date: string;
  duration_seconds: number;
  has_second_source: boolean;
}

/** The column a source's file path is stored in. */
const videoPathColumn = (source: SourceNum) =>
  source === 1 ? "video_path" : "video2_path";

/** A lecture's downloaded file for one source, or null. */
export const videoPathFor = (lec: Lecture, source: SourceNum) =>
  source === 1 ? lec.video_path : lec.video2_path;

export async function upsertLectures(subjectId: number, lectures: LectureData[]): Promise<void> {
  const db = await getDb();
  for (const l of lectures) {
    await db.execute(
      `INSERT INTO lectures
         (id, lesson_id, subject_id, title, date, duration_seconds, has_source2, synced_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, datetime('now'))
       ON CONFLICT(id) DO UPDATE SET
         title            = excluded.title,
         date             = excluded.date,
         duration_seconds = excluded.duration_seconds,
         has_source2      = excluded.has_source2,
         synced_at        = datetime('now')`,
      [
        l.id,
        l.lesson_id,
        subjectId,
        l.title,
        l.date,
        l.duration_seconds,
        l.has_second_source ? 1 : 0,
      ]
    );
  }
}

export async function getLecture(id: string): Promise<Lecture | null> {
  const db = await getDb();
  const rows = await db.select<Lecture[]>(`SELECT * FROM lectures WHERE id = $1`, [id]);
  return rows[0] ?? null;
}

export async function getLectures(subjectId: number): Promise<Lecture[]> {
  const db = await getDb();
  return db.select<Lecture[]>(
    `SELECT * FROM lectures WHERE subject_id = $1 ORDER BY date ASC`,
    [subjectId]
  );
}

export async function updateLectureVideoPath(
  id: string,
  path: string,
  source: SourceNum = 1
): Promise<void> {
  const db = await getDb();
  // The column name is one of two literals, never user input.
  await db.execute(`UPDATE lectures SET ${videoPathColumn(source)} = $1 WHERE id = $2`, [
    path,
    id,
  ]);
}

/** Forget a deleted download; progress, chapters and notes stay with the
 *  lecture. `source: null` clears both streams, like `echo360_delete_video`. */
export async function clearLectureVideoPath(
  id: string,
  source: SourceNum | null = null
): Promise<void> {
  const db = await getDb();
  const columns = source === null ? ([1, 2] as SourceNum[]) : [source];
  for (const s of columns) {
    // The column name is one of two literals, never user input.
    await db.execute(`UPDATE lectures SET ${videoPathColumn(s)} = NULL WHERE id = $1`, [id]);
  }
}

export async function updateLectureTranscriptPath(id: string, path: string): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE lectures SET transcript_path = $1 WHERE id = $2`, [path, id]);
}

/** Saving a position is the record of watching, so it stamps `last_watched_at`. */
export async function updateLectureProgress(id: string, seconds: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE lectures SET progress_seconds = $1, last_watched_at = datetime('now')
     WHERE id = $2`,
    [seconds, id],
  );
}

export async function markLectureComplete(id: string): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE lectures SET completed = 1, progress_seconds = duration_seconds,
                         last_watched_at = datetime('now')
     WHERE id = $1`,
    [id]
  );
}

/** The list's Done toggle. Marking keeps the position, since it may not have
 *  been watched; unmarking a lecture watched to its end (`rewind`) starts it
 *  over, or it would show nothing left and re-mark itself on the next save. */
export async function setLectureDone(id: string, done: boolean, rewind = false): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE lectures SET completed = $1,
                         progress_seconds = CASE WHEN $2 THEN 0 ELSE progress_seconds END
     WHERE id = $3`,
    [done ? 1 : 0, !done && rewind ? 1 : 0, id],
  );
}

/** The lecture after `lec` in its subject, Done or not: the earliest strictly
 *  later `date`, ties broken by title then id. What Up Next offers. */
export async function getNextLecture(
  lec: Pick<Lecture, "subject_id" | "date">,
): Promise<(Lecture & { subject_code: string }) | null> {
  const db = await getDb();
  const rows = await db.select<(Lecture & { subject_code: string })[]>(
    `SELECT l.*, s.code AS subject_code
       FROM lectures l
       JOIN subjects s ON s.id = l.subject_id
      WHERE l.subject_id = $1 AND l.date > $2
      ORDER BY l.date ASC, l.title ASC, l.id ASC
      LIMIT 1`,
    [lec.subject_id, lec.date],
  );
  return rows[0] ?? null;
}

/** The end job's columns and the transcript it reads. */
export type LectureEndRow = Pick<
  Lecture,
  "transcript_path" | "content_end_seconds" | "content_end_status" | "content_end_error"
>;

/** Read on its own, as `getChapterStatus` is. */
export async function getLectureEnd(lectureId: string): Promise<LectureEndRow | null> {
  const db = await getDb();
  const rows = await db.select<LectureEndRow[]>(
    `SELECT transcript_path, content_end_seconds, content_end_status, content_end_error
       FROM lectures WHERE id = $1`,
    [lectureId],
  );
  return rows[0] ?? null;
}

/** Lecture recordings across every subject — the calendar's third layer. */
export async function getAllLectures(): Promise<
  (Lecture & { subject_code: string })[]
> {
  const db = await getDb();
  return db.select<(Lecture & { subject_code: string })[]>(
    `SELECT l.*, s.code AS subject_code
       FROM lectures l
       JOIN subjects s ON s.id = l.subject_id
      ORDER BY l.date ASC`,
  );
}
