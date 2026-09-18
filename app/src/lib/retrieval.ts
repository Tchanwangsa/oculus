import { invoke } from "@tauri-apps/api/core";

import { getDb, type DbFile } from "./db";

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
  /**
   * True when a `.emb.json` in the current embedding space was already beside
   * the PDF, so nothing was sent to the backend. The page rows are still
   * upserted — an artifact on disk is no promise the database can see it.
   */
  skipped: boolean;
}

/**
 * What the index holds, split into what can be searched **now** and what is
 * merely stored.
 *
 * The split is the point of the type. Vectors from two models share a table, a
 * width and a dot product, and share no geometry at all, so only the ones from
 * the model that embeds the query are ever scanned. A library embedded by a
 * retired model therefore reads as `pages_embedded: 0` with a large
 * `pages_stored` — and that is the truth, not a bug: those pages are not
 * searchable until they are re-embedded.
 */
export interface IndexStats {
  /** Searchable now: pages and files with a vector in the current space. */
  files_embedded: number;
  pages_embedded: number;
  pages_with_markdown: number;
  /** The space the counts above are counted in. */
  model: string | null;
  dim: number | null;
  /** Every vector in the table, whichever model wrote it. */
  files_stored: number;
  pages_stored: number;
  /** Stored but not searchable. These re-embed; nothing migrates them. */
  pages_stale: number;
  /** Which models those came from, so a message can name them. */
  stale_models: string[];
}

/**
 * Embed every page of a PDF and store the vectors against `fileId`.
 *
 * `relativePath` is relative to the app data dir, same as `read_course_file`.
 *
 * `subjectId` is not used to find the file — `relativePath` already does that.
 * It is there because the call narrates itself over `embed-status`, and the
 * pipeline row that listens is keyed on `(subjectId, relativePath)` exactly as
 * the parse path's rows are. Rust emits `queued`, a `running` per finished
 * batch of pages, and one terminal `done` / `error`; see
 * `app/src-tauri/src/embed/events.rs`.
 */
export function embedFile(
  fileId: number,
  subjectId: number,
  relativePath: string,
  force = false,
): Promise<IngestSummary> {
  return invoke<IngestSummary>("embed_file", { fileId, subjectId, relativePath, force });
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

/**
 * Is there an embedder to send work to right now?
 *
 * The gate in front of auto-embed, and it asks Rust rather than guessing: a
 * key in the keychain is the cloud engine's answer, and a selected engine that
 * this build does not ship (`Engine::Local`) has none whatever the keychain
 * says. Both halves have to hold, because a queue that fills against a
 * backend which cannot run produces one failed row per parsed file.
 */
export async function embedReady(): Promise<boolean> {
  try {
    const settings = await invoke<{
      engine: string;
      credentials_ready: boolean;
      engines: Array<{ id: string; available: boolean }>;
    }>("embed_settings");
    const engine = settings.engines.find((e) => e.id === settings.engine);
    return settings.credentials_ready && (engine?.available ?? true);
  } catch {
    return false;
  }
}

export function embeddingStats(): Promise<IndexStats> {
  return invoke<IndexStats>("embedding_stats");
}

/**
 * Is something account-wide stopping the run — a spent Voyage allowance, or the
 * spend limit in Settings → Library? The reason when yes, `null` when the next
 * file may go ahead.
 *
 * Asked between files, never before one: it is cheap (it reads
 * `voyage-usage.json`, constructs no client and sends nothing) but it is a
 * *reaction*, not a precondition. The run is allowed to try and be refused;
 * what it is not allowed to do is try 166 times for the same reason.
 */
export function embedBlocked(): Promise<string | null> {
  return invoke<string | null>("embed_blocked");
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
 * that have no *usable* embeddings yet.
 *
 * Parsing must have run first — not because embedding needs the markdown (it
 * works off the page image), but because a hit with no markdown has nothing to
 * hydrate an answer from. `'quality'` is the one finished-parse marker; the
 * fast tier it was named against is gone.
 *
 * **This asks the `pages` table, not `files.embed_status`, and the difference
 * is not cosmetic.** `embed_status` is a sticky flag written when a file was
 * embedded, with no memory of *which space* it was embedded into — so after
 * the move off the local model, every file in the library said `'done'` while
 * holding vectors no query can be compared against. A predicate built on that
 * flag reports an empty backlog over a library where nothing is searchable.
 * Counting current-space page vectors instead makes the answer follow the
 * space, which is also what `embed::is_embedded` does in Rust: a file is
 * finished when its pages are covered *by this model*, and partial coverage
 * (a rate limit that stopped a document halfway) is unfinished rather than
 * silently permanent.
 */
export async function getUnembeddedPdfs(subjectId?: number): Promise<DbFile[]> {
  const db = await getDb();
  // The space comes from the backend rather than a constant here: which model
  // is current is Rust's answer to give, and hardcoding it in the WebView is
  // how the two drift apart.
  const { model, dim } = await embeddingStats();
  const scope = subjectId != null ? `AND f.subject_id = $3` : ``;
  return db.select<DbFile[]>(
    `SELECT f.* FROM files f
     WHERE lower(f.file_type) IN ('pdf', 'pptx', 'docx', 'ppt', 'doc')
       AND f.parse_status = 'quality'
       AND (SELECT COUNT(*) FROM pages p
             WHERE p.file_id = f.id AND p.embedding IS NOT NULL
               AND p.embed_model = $1 AND p.embed_dim = $2)
           < max((SELECT COUNT(*) FROM pages p2 WHERE p2.file_id = f.id), 1)
       ${scope}
     ORDER BY f.relative_path ASC`,
    subjectId != null ? [model, dim, subjectId] : [model, dim],
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
