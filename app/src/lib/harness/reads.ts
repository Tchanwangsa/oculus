import { getDb, getSetting } from "@/lib/db";
import type { Provider } from "./providers";
import type { HarnessThread, RateWindow } from "./types";

/** A thread's `settled_at`: when its latest run ended. Only a run that ran
 *  settles a thread: one that waits in the queue, or left it without
 *  starting (`queued`, `cancelled`), is not an answer to come back to. */
const SETTLED_AT =
  "(SELECT MAX(r.ended_at) FROM harness_runs r WHERE r.thread_id = t.id" +
  " AND r.execution_status IN ('completed', 'interrupted', 'failed') AND r.initiating_item_id IS NOT NULL) AS settled_at";

/** A thread's `pending_requests`: those waiting on the student, counted as
 *  Rust counts them for each request event (`requests::store::waiting`). */
const PENDING_REQUESTS =
  "(SELECT COUNT(*) FROM harness_requests q WHERE q.thread_id = t.id" +
  " AND q.status = 'pending' AND q.responder IN ('live', 'message')) AS pending_requests";

const PENDING_QUESTIONS =
  "(SELECT COUNT(*) FROM harness_requests q WHERE q.thread_id = t.id" +
  " AND q.kind = 'question' AND q.status = 'pending' AND q.responder IN ('live', 'message')) AS pending_questions";

/** The thread list's read, newest first; `$1` is the limit. */
export const THREADS_SQL = `SELECT t.*, ${SETTLED_AT}, ${PENDING_REQUESTS}, ${PENDING_QUESTIONS} FROM harness_threads t ORDER BY t.updated_at DESC, t.id DESC LIMIT $1`;

export async function getHarnessThreads(limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(THREADS_SQL, [limit]);
}

/** One lecture's threads — its own query, since `getHarnessThreads` caps
 *  library-wide and would drop a lecture's older ones. */
export async function getLectureThreads(lectureId: string, limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(
    `SELECT t.*, ${SETTLED_AT}, ${PENDING_REQUESTS}, ${PENDING_QUESTIONS} FROM harness_threads t WHERE t.lecture_id = $1
      ORDER BY t.updated_at DESC, t.id DESC LIMIT $2`,
    [lectureId, limit],
  );
}

/** Each thread's latest answer, flattened to one line of plain text — the
 *  empty Chat page's recent list. An answer is a run's selected one, or a
 *  reply with no run (history from before runs, a turn the provider started
 *  itself), as thread naming reads it. Threads with none yet are absent. */
/** `getThreadPreviews`' read for `count` thread ids. It goes by rows, so a
 *  run with none (one that waits, or was cancelled) is never an answer. */
export function previewsSql(count: number): string {
  const marks = Array.from({ length: count }, (_, i) => `$${i + 1}`).join(", ");
  return `SELECT thread_id, content FROM harness_items
      WHERE id IN (SELECT MAX(i.id) FROM harness_items i
                     LEFT JOIN harness_runs r ON r.id = i.run_id
                    WHERE i.kind = 'assistant' AND (i.run_id IS NULL OR r.answer_item_id = i.id)
                      AND i.thread_id IN (${marks})
                    GROUP BY i.thread_id)`;
}

export async function getThreadPreviews(ids: number[]): Promise<Map<number, string>> {
  if (!ids.length) return new Map();
  const db = await getDb();
  const rows = await db.select<{ thread_id: number; content: string | null }[]>(previewsSql(ids.length), ids);
  const out = new Map<number, string>();
  for (const r of rows) {
    const text = (r.content ?? "")
      .replace(/```[\s\S]*?```/g, " ")
      .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
      .replace(/^\s*(#+|>+|[-*+]|\d+\.)\s+/gm, "")
      // Not `_`: it is inside paths and course codes far more than emphasis.
      .replace(/[*`~|$]+/g, "")
      .replace(/\s+/g, " ")
      .trim();
    if (text) out.set(r.thread_id, text);
  }
  return out;
}

export async function getHarnessRateLimits(provider: Provider): Promise<RateWindow[]> {
  const raw = await getSetting(`harness_rate_limits_${provider}`);
  if (!raw) return [];
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}
