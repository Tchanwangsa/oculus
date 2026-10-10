import { getDb } from "./connection";
import { PDF_BACKED_SQL_LIST, PIPELINE_SQL_LIST } from "@/lib/files/fileTypes";

/** One file's stages as the DB holds them. */
export interface PdfPipelineRow {
  subject_id: number;
  relative_path: string;
  parse_status: string | null;
  embed_status: string | null;
  /** Rewritten by every sync, changed bytes or not — only a fallback for
   *  the download time below. */
  scraped_at: string | null;
  first_seen_at: string | null;
  content_changed_at: string | null;
  parsed_at: string | null;
  embedded_at: string | null;
}

async function pipelineRows(types: string): Promise<PdfPipelineRow[]> {
  const db = await getDb();
  return db.select<PdfPipelineRow[]>(
    `SELECT subject_id, relative_path, parse_status, embed_status,
            scraped_at, first_seen_at, content_changed_at, parsed_at, embedded_at
     FROM files
     WHERE lower(file_type) IN ${types}
     ORDER BY relative_path ASC`,
  );
}

/** Every PDF-backed file's stages — the parse counts in Settings. */
export function getPdfPipelineRows(): Promise<PdfPipelineRow[]> {
  return pipelineRows(PDF_BACKED_SQL_LIST);
}

/** Every file with a File Activity row (PDF-backed, and spreadsheets) —
 *  seeds the Sync page's pipeline table. */
export function getPipelineRows(): Promise<PdfPipelineRow[]> {
  return pipelineRows(PIPELINE_SQL_LIST);
}

/** status: 'queued' | 'running' | 'quality' | 'error'; `'quality'` is the
 *  one terminal success. */
export async function setParseStatus(
  subjectId: number,
  relativePath: string,
  status: string,
): Promise<boolean> {
  const db = await getDb();
  const setParsedAt = status === "quality";
  const result = await db.execute(
    `UPDATE files SET parse_status = $1${setParsedAt ? ", parsed_at = datetime('now')" : ""}
     WHERE subject_id = $2 AND relative_path = $3`,
    [status, subjectId, relativePath],
  );
  return result.rowsAffected > 0;
}

export interface EmbedCoverageRow {
  relative_path: string;
  /** Page rows the parse wrote; 0 means unparsed, so never covered. */
  pages_total: number;
  pages_current: number;
}

/** How much of each PDF is embedded in the given space — page-vector
 *  coverage, not `files.embed_status` (see `getUnembeddedPdfs`). */
export async function getEmbedCoverage(
  model: string | null,
  dim: number | null,
): Promise<EmbedCoverageRow[]> {
  if (!model || dim == null) return [];
  const db = await getDb();
  return db.select<EmbedCoverageRow[]>(
    `SELECT f.relative_path,
            (SELECT COUNT(*) FROM pages p WHERE p.file_id = f.id) AS pages_total,
            (SELECT COUNT(*) FROM pages p
              WHERE p.file_id = f.id AND p.embedding IS NOT NULL
                AND p.embed_model = $1 AND p.embed_dim = $2) AS pages_current
     FROM files f
     WHERE lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}`,
    [model, dim],
  );
}

/** status: 'done' | 'error'. Mostly for the failure: success shows in page
 *  vectors, but a failure leaves no other trace across a restart. Rust also
 *  writes `'done'` when an ingest commits; `'queued'` is the app queue's own
 *  mark (`markEmbedQueued`). */
export async function setEmbedStatus(
  subjectId: number,
  relativePath: string,
  status: string,
): Promise<void> {
  const db = await getDb();
  const setEmbeddedAt = status === "done";
  await db.execute(
    `UPDATE files SET embed_status = $1${setEmbeddedAt ? ", embedded_at = datetime('now')" : ""}
     WHERE subject_id = $2 AND relative_path = $3`,
    [status, subjectId, relativePath],
  );
}

/** SQLite's bound-variable ceiling is far above this; chunking keeps a whole
 *  backlog's ids from building one enormous statement. */
const ID_CHUNK = 500;

async function updateByIds(sql: (placeholders: string) => string, ids: number[]): Promise<void> {
  if (ids.length === 0) return;
  const db = await getDb();
  for (let i = 0; i < ids.length; i += ID_CHUNK) {
    const chunk = ids.slice(i, i + ID_CHUNK);
    await db.execute(sql(chunk.map((_, j) => `$${j + 1}`).join(", ")), chunk);
  }
}

/** Record files as waiting in the app's embed queue, so the queue survives a
 *  restart (`restoreIndexQueue` in stores/sync/indexStore.ts). */
export function markEmbedQueued(fileIds: number[]): Promise<void> {
  return updateByIds(
    (ids) => `UPDATE files SET embed_status = 'queued' WHERE id IN (${ids})`,
    fileIds,
  );
}

/** Take files out of the persisted queue. Only a row still saying `queued`
 *  is touched: a finished embed's `done` / `error` stands. */
export function clearEmbedQueued(fileIds: number[]): Promise<void> {
  return updateByIds(
    (ids) => `UPDATE files SET embed_status = NULL
              WHERE embed_status = 'queued' AND id IN (${ids})`,
    fileIds,
  );
}

export interface QueuedEmbedRow {
  id: number;
  subject_id: number;
  relative_path: string;
  filename: string;
  /** A parsed, PDF-backed file — what `getUnembeddedPdfs` would offer. */
  embeddable: number;
  pages_total: number;
  pages_current: number;
}

/** Files the persisted embed queue holds, with their coverage in the given
 *  space. Ordered by parse time, the order a parse-fed queue filled in. */
export async function getQueuedEmbedRows(
  model: string | null,
  dim: number | null,
): Promise<QueuedEmbedRow[]> {
  const db = await getDb();
  return db.select<QueuedEmbedRow[]>(
    `SELECT f.id, f.subject_id, f.relative_path, f.filename,
            (lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}
              AND f.parse_status = 'quality') AS embeddable,
            (SELECT COUNT(*) FROM pages p WHERE p.file_id = f.id) AS pages_total,
            (SELECT COUNT(*) FROM pages p
              WHERE p.file_id = f.id AND p.embedding IS NOT NULL
                AND p.embed_model = $1 AND p.embed_dim = $2) AS pages_current
     FROM files f
     WHERE f.embed_status = 'queued'
     ORDER BY f.parsed_at IS NULL, f.parsed_at, f.relative_path`,
    [model, dim],
  );
}

/** Bulk-set parse status by relative_path, for disk reconciliation. A
 *  recorded `parsed_at` is kept: the parse happened then, not when the disk
 *  check noticed it. */
export async function setParseStatusByPath(
  entries: Array<[string, string]>,
): Promise<void> {
  if (entries.length === 0) return;
  const db = await getDb();
  for (const [relativePath, status] of entries) {
    await db.execute(
      `UPDATE files SET parse_status = $1, parsed_at = COALESCE(parsed_at, datetime('now'))
       WHERE relative_path = $2 AND (parse_status IS NULL OR parse_status != $1)`,
      [status, relativePath],
    );
  }
}
