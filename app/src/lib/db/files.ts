import { getDb } from "./connection";
import { likeEscape } from "./match";
import type { DbFile } from "./types";
import { parsedMdSource } from "@/lib/files/fileTypes";

export async function upsertFile(
  subjectId: number,
  filename: string,
  relativePath: string,
  fileType: string,
  sizeBytes?: number,
  category?: string,
  canvasId?: number,
  sourceUrl?: string,
  sourceCreatedAt?: string | null,
  sourceUpdatedAt?: string | null,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes, category, canvas_id, source_url, first_seen_at, source_created_at, source_updated_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, datetime('now'), $9, $10)
     ON CONFLICT(subject_id, relative_path) DO UPDATE SET
       filename   = excluded.filename,
       file_type  = excluded.file_type,
       size_bytes = excluded.size_bytes,
       category   = excluded.category,
       canvas_id  = excluded.canvas_id,
       source_url = excluded.source_url,
       source_created_at = COALESCE(excluded.source_created_at, files.source_created_at),
       source_updated_at = COALESCE(excluded.source_updated_at, files.source_updated_at),
       scraped_at = datetime('now')`,
    [subjectId, filename, relativePath, fileType, sizeBytes ?? null,
     category ?? null, canvasId ?? null, sourceUrl ?? null,
     sourceCreatedAt ?? null, sourceUpdatedAt ?? null]
  );
}

/** A scrape write changed this file's bytes ('new'/'updated', never
 *  'unchanged') — brings the unseen dot back. */
export async function markFileContentChanged(
  subjectId: number,
  relativePath: string,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE files SET content_changed_at = datetime('now')
     WHERE subject_id = $1 AND relative_path = $2`,
    [subjectId, relativePath],
  );
}

/** Forget a file's parse and embed state after a re-scrape changed its bytes,
 *  so both stages re-run and no stale page vector is ranked. Rust purges the
 *  on-disk artifacts. */
export async function resetFilePipeline(
  subjectId: number,
  relativePath: string,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `DELETE FROM pages WHERE file_id IN
       (SELECT id FROM files WHERE subject_id = $1 AND relative_path = $2)`,
    [subjectId, relativePath],
  );
  await db.execute(
    `UPDATE files SET parse_status = NULL, parsed_at = NULL,
                      embed_status = NULL, embedded_at = NULL
     WHERE subject_id = $1 AND relative_path = $2`,
    [subjectId, relativePath],
  );
}

/** Drop a file's row with its indexed pages and a note's versions. Those are
 *  deleted explicitly rather than trusting `ON DELETE CASCADE`, which only
 *  fires on a connection with `foreign_keys` on. */
export async function deleteFileRow(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM pages WHERE file_id = $1`, [id]);
  await db.execute(`DELETE FROM document_versions WHERE file_id = $1`, [id]);
  await db.execute(`DELETE FROM files WHERE id = $1`, [id]);
}

export async function markFileAccessed(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE files SET last_accessed_at = datetime('now') WHERE id = $1`, [id]);
}

/** Record that a file's bytes changed under the app's hand (editor save, or
 *  the chat agent). `seen` — the student's own save — also stamps
 *  `last_accessed_at`, so the unseen dot lights only for changes they didn't make. */
export async function touchFileRow(
  id: number,
  sizeBytes: number,
  seen: boolean,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    seen
      ? `UPDATE files SET size_bytes = $1, modified_at = datetime('now'),
                          content_changed_at = datetime('now'),
                          last_accessed_at = datetime('now')
         WHERE id = $2`
      : `UPDATE files SET size_bytes = $1, modified_at = datetime('now'),
                          content_changed_at = datetime('now')
         WHERE id = $2`,
    [sizeBytes, id],
  );
}

/** Follow a file that moved on disk, keeping its id so `pages` stay attached. */
export async function renameFileRow(
  id: number,
  filename: string,
  relativePath: string,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE files SET filename = $1, relative_path = $2, modified_at = datetime('now')
     WHERE id = $3`,
    [filename, relativePath, id],
  );
}

/** One file by its library path (`courses/<subject>/…`), which includes the
 *  subject folder and so needs no subject id. */
export async function getFileByRelativePath(
  relativePath: string,
): Promise<DbFile | null> {
  const db = await getDb();
  const rows = await db.select<DbFile[]>(
    `SELECT * FROM files WHERE relative_path = $1 LIMIT 1`,
    [relativePath],
  );
  return rows[0] ?? null;
}

/** A subject's file by its Canvas file id: a `<canvas-video>` card's
 *  downloaded video. */
export async function getFileByCanvasId(
  subjectId: number,
  canvasId: number,
): Promise<DbFile | null> {
  const db = await getDb();
  const rows = await db.select<DbFile[]>(
    `SELECT * FROM files WHERE subject_id = $1 AND canvas_id = $2 LIMIT 1`,
    [subjectId, canvasId],
  );
  return rows[0] ?? null;
}

/** Files whose library path ends in `/<tail>` — a course-relative path or a
 *  bare filename an agent cited. At most two, so a caller can tell a unique
 *  hit from an ambiguous one. */
export async function findFilesByTail(tail: string): Promise<DbFile[]> {
  const db = await getDb();
  return db.select<DbFile[]>(
    `SELECT * FROM files WHERE relative_path LIKE $1 ESCAPE '\\' LIMIT 2`,
    [`%/${likeEscape(tail)}`],
  );
}

export async function getFilesForSubject(subjectId: number): Promise<DbFile[]> {
  const db = await getDb();
  return db.select<DbFile[]>(
    `SELECT * FROM files WHERE subject_id = $1 ORDER BY relative_path ASC`,
    [subjectId]
  );
}

/** Which of these library paths (`courses/…`) the files table still holds: a
 *  row at the path, a parsed `.md` whose source has one, or for a folder
 *  (`…/`) any row under it. One query for the files, one per folder. */
export async function presentLibraryPaths(paths: readonly string[]): Promise<Set<string>> {
  const present = new Set<string>();
  const folders = paths.filter((p) => p.endsWith("/"));
  const files = paths.filter((p) => !p.endsWith("/"));
  const db = await getDb();
  if (files.length) {
    const candidates = [...new Set(files.flatMap((p) => [p, parsedMdSource(p) ?? p]))];
    const rows = await db.select<{ relative_path: string }[]>(
      `SELECT relative_path FROM files WHERE relative_path IN (${candidates.map((_, i) => `$${i + 1}`).join(", ")})`,
      candidates,
    );
    const found = new Set(rows.map((r) => r.relative_path));
    for (const p of files) if (found.has(p) || found.has(parsedMdSource(p) ?? p)) present.add(p);
  }
  for (const folder of folders) {
    const rows = await db.select<{ one: number }[]>(
      `SELECT 1 AS one FROM files WHERE relative_path LIKE $1 ESCAPE '\\' LIMIT 1`,
      [`${likeEscape(folder)}%`],
    );
    if (rows.length) present.add(folder);
  }
  return present;
}
