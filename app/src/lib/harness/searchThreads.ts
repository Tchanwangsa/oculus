import { getDb } from "@/lib/db";
import { HIT_CLOSE, HIT_OPEN } from "./snippet";
import type { HarnessThread } from "./types";

export interface ThreadSearchResult extends HarnessThread {
  /** Plain text with matched spans between `HIT_OPEN` and `HIT_CLOSE`, never HTML. */
  snippet: string;
  subject_code: string | null;
  subject_name: string | null;
  score: number;
}

/** Quote each token before adding our prefix operator. User punctuation,
 *  operators, column selectors and quotes never become FTS syntax. */
export function threadSearchBindings(q: string): [string, string] | null {
  const text = q.trim();
  const terms = text.match(/[\p{L}\p{N}\p{M}]+/gu);
  if (!terms?.length) return null;
  const match = terms.map((term) => `"${term}"*`).join(" AND ");
  const like = `%${text.replace(/[\\%_]/g, "\\$&")}%`;
  return [match, like];
}

// Materialize before the window function: FTS auxiliary functions must run
// directly against the MATCH cursor, not in an aggregate/window context.
export const SEARCH_THREADS_SQL = `WITH message_hits AS MATERIALIZED (
  SELECT i.thread_id, i.id AS item_id, bm25(harness_items_fts) AS score,
    snippet(harness_items_fts, 0, '${HIT_OPEN}', '${HIT_CLOSE}', '…', 24) AS snippet
  FROM harness_items_fts JOIN harness_items i ON i.id = harness_items_fts.rowid
  WHERE harness_items_fts MATCH $1 AND i.kind IN ('user', 'assistant')
), hits AS (
  SELECT thread_id, item_id, score, snippet FROM message_hits
  UNION ALL
  SELECT t.id, 0, -1000000.0, '${HIT_OPEN}' || t.title || '${HIT_CLOSE}'
  FROM harness_threads t WHERE t.title LIKE $2 ESCAPE '\\'
), best AS (
  SELECT *, ROW_NUMBER() OVER (PARTITION BY thread_id ORDER BY score, item_id DESC) AS position FROM hits
)
SELECT t.*, s.code AS subject_code, s.name AS subject_name, best.snippet, best.score
FROM best JOIN harness_threads t ON t.id = best.thread_id
LEFT JOIN subjects s ON s.id = t.subject_id
WHERE best.position = 1
ORDER BY best.score, julianday(t.updated_at) DESC, t.id DESC LIMIT 50`;

/** Includes completed threads. Empty/punctuation-only queries read nothing. */
export async function searchThreads(q: string): Promise<ThreadSearchResult[]> {
  const bindings = threadSearchBindings(q);
  if (!bindings) return [];
  const db = await getDb();
  return db.select<ThreadSearchResult[]>(SEARCH_THREADS_SQL, bindings);
}
