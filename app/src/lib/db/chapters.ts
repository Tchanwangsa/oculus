import { getDb } from "./connection";

/** A `lecture_chapters` row (`app/src-tauri/src/lectures/chapters/`). No end column:
 *  a chapter runs until the next starts (`chapterSpans` in lib/lectures/index.ts). */
export interface Chapter {
  lecture_id: string;
  idx: number;
  start_seconds: number;
  title: string;
  summary: string;
}

export async function getChapters(lectureId: string): Promise<Chapter[]> {
  const db = await getDb();
  return db.select<Chapter[]>(
    `SELECT * FROM lecture_chapters WHERE lecture_id = $1 ORDER BY idx ASC`,
    [lectureId],
  );
}

/** Read on its own: the player's `lecture` prop is a snapshot not re-read
 *  when a run lands. */
export async function getChapterStatus(
  lectureId: string,
): Promise<{ chapter_status: string | null; chapter_error: string | null } | null> {
  const db = await getDb();
  const rows = await db.select<
    { chapter_status: string | null; chapter_error: string | null }[]
  >(`SELECT chapter_status, chapter_error FROM lectures WHERE id = $1`, [lectureId]);
  return rows[0] ?? null;
}
