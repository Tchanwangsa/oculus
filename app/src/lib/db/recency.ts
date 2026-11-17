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

/** A file on Drive's Recent list: the row, its subject's code, what put it
 *  there, and when (UTC `datetime('now')`). */
export interface RecentDriveFile extends LibraryFileHit {
  recent_at: string;
  recent_why: "opened" | "changed";
}

/** Drive's Recent: files opened in the last `days`, and the student's own
 *  files made or changed then (a save, an upload, a move, a file an agent
 *  wrote), newest first. Synced changes are left out: a sync rewrites too
 *  many rows to be anyone's recent work. */
export async function getRecentDriveFiles(limit = 100, days = 30): Promise<RecentDriveFile[]> {
  const db = await getDb();
  return db.select<RecentDriveFile[]>(
    `SELECT * FROM (
       SELECT f.*, s.code AS subject_code,
              MAX(COALESCE(f.last_accessed_at, ''), changed) AS recent_at,
              CASE WHEN COALESCE(f.last_accessed_at, '') >= changed THEN 'opened' ELSE 'changed' END AS recent_why
         FROM (SELECT *,
                      CASE WHEN category IN ('upload', 'document', 'canvas', 'notebook')
                           THEN MAX(COALESCE(modified_at, ''), COALESCE(first_seen_at, ''))
                           ELSE '' END AS changed
                 FROM files) f
         JOIN subjects s ON s.id = f.subject_id
     )
     WHERE recent_at >= datetime('now', $2)
     ORDER BY recent_at DESC
     LIMIT $1`,
    [limit, `-${days} days`],
  );
}
