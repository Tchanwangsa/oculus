import { invoke } from "@tauri-apps/api/core";

import { getDb, type DbFile } from "../db";
import { PDF_BACKED_SQL_LIST } from "../files/fileTypes";

/**
 * Semantic page retrieval. The index embeds page images, not extracted text —
 * see docs/retrieval.md.
 */

interface IngestSummary {
  file_id: number;
  pages_embedded: number;
  pages_with_markdown: number;
  model: string;
  dim: number;
  /** A current-space `.emb.json` was already on disk; rows still upserted. */
  skipped: boolean;
}

/** Searchable (current model's vectors) vs stored (any model's): another
 *  model's vectors share the table but not the geometry. */
interface IndexStats {
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
  /** Stored but not searchable; these re-embed, nothing migrates them. */
  pages_stale: number;
  stale_models: string[];
}

/**
 * Embed every page of a PDF and store the vectors against `fileId`.
 * `relativePath` (relative to the data dir) finds the file; `subjectId` only
 * keys the `embed-status` events the pipeline row listens to
 * (`app/src-tauri/src/embed/events.rs`).
 */
export function embedFile(
  fileId: number,
  subjectId: number,
  relativePath: string,
  force = false,
): Promise<IngestSummary> {
  return invoke<IngestSummary>("embed_file", { fileId, subjectId, relativePath, force });
}

/** A credential and an available engine; otherwise the queue fills with one
 *  failed row per parsed file. */
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
 * The reason something account-wide (spent Voyage allowance, spend limit) is
 * stopping the run, or `null`. Asked between files, not before one: the run
 * may try and be refused, but must not retry every file for the same reason.
 */
export function embedBlocked(): Promise<string | null> {
  return invoke<string | null>("embed_blocked");
}

/** Parsed PDF-backed files not fully embedded in the current space. Reads
 *  `pages`, not `files.embed_status` (which has no model); as `embed::is_embedded`. */
export async function getUnembeddedPdfs(subjectId?: number): Promise<DbFile[]> {
  const db = await getDb();
  // The current space is Rust's answer; a constant here would drift from it.
  const { model, dim } = await embeddingStats();
  const scope = subjectId != null ? `AND f.subject_id = $3` : ``;
  return db.select<DbFile[]>(
    `SELECT f.* FROM files f
     WHERE lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}
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
