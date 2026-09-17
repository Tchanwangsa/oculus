import { invoke } from "@tauri-apps/api/core";

import { getDb, type DbFile } from "./db";
import { PDF_BACKED_SQL_LIST } from "./fileTypes";

/**
 * Semantic page retrieval.
 *
 * The index is built from page images rather than scraped text — on formula
 * slides and screenshots, text extraction returns garbage and image embeddings
 * roughly double recall. Each hit carries the page's markdown (what an answer
 * is written from) and its (file, page) ref (what a citation deep-links to).
 */

export interface SearchHit {
  file_id: number;
  page_no: number;
  /** Cosine similarity. Vectors are stored unit-length, so this is a dot product. */
  score: number;
  filename: string;
  relative_path: string;
  subject_id: number;
  markdown: string;
}

export interface IngestSummary {
  file_id: number;
  pages_embedded: number;
  pages_with_markdown: number;
  model: string;
  dim: number;
  /** True when the sidecar found an up-to-date sidecar file and did no work. */
  skipped: boolean;
}

export interface IndexStats {
  files_embedded: number;
  pages_embedded: number;
  pages_with_markdown: number;
  model: string | null;
  dim: number | null;
}

/**
 * Embed every page of a PDF and store the vectors against `fileId`.
 *
 * `relativePath` is relative to the app data dir, same as `read_course_file`.
 */
export function embedFile(
  fileId: number,
  relativePath: string,
  force = false,
): Promise<IngestSummary> {
  return invoke<IngestSummary>("embed_file", { fileId, relativePath, force });
}

/**
 * Embed everything parsed but not yet indexed, one at a time.
 *
 * Serial on purpose: the sidecar holds a single model on the GPU, so parallel
 * calls would queue there anyway while multiplying peak memory.
 */
export async function embedPending(
  subjectId?: number,
  onProgress?: (done: number, total: number, filename: string) => void,
): Promise<{ files: number; pages: number; errors: string[] }> {
  const pending = await getUnembeddedPdfs(subjectId);
  const errors: string[] = [];
  let pages = 0;
  let done = 0;

  for (const f of pending) {
    onProgress?.(done, pending.length, f.filename);
    try {
      const summary = await embedFile(f.id, f.relative_path);
      pages += summary.pages_embedded;
    } catch (e) {
      errors.push(`${f.filename}: ${e}`);
    }
    done += 1;
  }
  onProgress?.(done, pending.length, "");
  return { files: done - errors.length, pages, errors };
}

/** Rank indexed pages against a natural-language question. */
export function searchPages(
  query: string,
  limit = 5,
  subjectId?: number,
): Promise<SearchHit[]> {
  return invoke<SearchHit[]>("search_pages", {
    query,
    limit,
    subjectId: subjectId ?? null,
  });
}

export function embeddingStats(): Promise<IndexStats> {
  return invoke<IndexStats>("embedding_stats");
}

// ── Queries against the pages table ──────────────────────────────────────────

export interface DbPage {
  id: number;
  file_id: number;
  page_no: number;
  markdown: string;
  embed_model: string | null;
  embed_dim: number | null;
  embedded_at: string | null;
}

/** Page rows for one file, without the blobs — those are only useful to Rust. */
export async function getPagesForFile(fileId: number): Promise<DbPage[]> {
  const db = await getDb();
  return db.select<DbPage[]>(
    `SELECT id, file_id, page_no, markdown, embed_model, embed_dim, embedded_at
     FROM pages WHERE file_id = $1 ORDER BY page_no ASC`,
    [fileId],
  );
}

/**
 * Parsed PDF-backed files (PDFs and Office docs with a converted sibling)
 * that have no embeddings yet.
 *
 * Parsing must have run first — not because embedding needs the markdown (it
 * works off the page image), but because a hit with no markdown has nothing to
 * hydrate an answer from.
 */
export async function getUnembeddedPdfs(subjectId?: number): Promise<DbFile[]> {
  const db = await getDb();
  const scope = subjectId != null ? `AND f.subject_id = $1` : ``;
  return db.select<DbFile[]>(
    `SELECT f.* FROM files f
     WHERE lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}
       AND f.parse_status IN ('fast', 'quality')
       AND (f.embed_status IS NULL OR f.embed_status != 'done')
       ${scope}
     ORDER BY f.relative_path ASC`,
    subjectId != null ? [subjectId] : [],
  );
}

/** Hydrate one page's markdown — the join that turns a ranked ref into text. */
export async function getPageMarkdown(
  fileId: number,
  pageNo: number,
): Promise<string | null> {
  const db = await getDb();
  const rows = await db.select<{ markdown: string }[]>(
    `SELECT markdown FROM pages WHERE file_id = $1 AND page_no = $2`,
    [fileId, pageNo],
  );
  return rows[0]?.markdown ?? null;
}
