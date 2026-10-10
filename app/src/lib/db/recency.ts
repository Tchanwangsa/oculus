import { getDb } from "./connection";
import type { Lecture } from "./lectures";
import type { LibraryFileHit } from "./search";

/** Lectures in progress, most recently watched first. The `> 5` must match
 *  the "started" threshold in `progressLabel` (lib/lectures/end.ts). */
export async function getRecentlyWatchedLectures(
  limit = 8,
): Promise<(Lecture & { subject_code: string })[]> {
  const db = await getDb();
  return db.select<(Lecture & { subject_code: string })[]>(
    `SELECT l.*, s.code AS subject_code
       FROM lectures l
       JOIN subjects s ON s.id = l.subject_id
      WHERE l.completed = 0
        AND l.last_watched_at IS NOT NULL
        AND l.progress_seconds > 5
      ORDER BY l.last_watched_at DESC
      LIMIT $1`,
    [limit],
  );
}

/** Recently opened files, newest first, in the palette's shape. */
export async function getRecentlyAccessedFiles(limit = 8): Promise<LibraryFileHit[]> {
  const db = await getDb();
  return db.select<LibraryFileHit[]>(
    `SELECT f.*, s.code AS subject_code
       FROM files f
       JOIN subjects s ON s.id = f.subject_id
      WHERE f.last_accessed_at IS NOT NULL
      ORDER BY f.last_accessed_at DESC
      LIMIT $1`,
    [limit],
  );
}
