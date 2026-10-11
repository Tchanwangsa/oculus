import { getDb, getSetting } from "@/lib/db";
import type { Provider } from "./providers";
import type { HarnessItem, HarnessThread, RateWindow } from "./types";

export async function getHarnessThreads(limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(
    `SELECT * FROM harness_threads ORDER BY updated_at DESC, id DESC LIMIT $1`,
    [limit],
  );
}

/** One lecture's threads — its own query, since `getHarnessThreads` caps
 *  library-wide and would drop a lecture's older ones. */
export async function getLectureThreads(lectureId: string, limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(
    `SELECT * FROM harness_threads WHERE lecture_id = $1
      ORDER BY updated_at DESC, id DESC LIMIT $2`,
    [lectureId, limit],
  );
}

/** Each thread's latest reply, flattened to one line of plain text — the
 *  empty Chat page's recent list. Threads with no reply yet are absent. */
export async function getThreadPreviews(ids: number[]): Promise<Map<number, string>> {
  if (!ids.length) return new Map();
  const db = await getDb();
  const marks = ids.map((_, i) => `$${i + 1}`).join(", ");
  const rows = await db.select<{ thread_id: number; content: string | null }[]>(
    `SELECT thread_id, content FROM harness_items
      WHERE id IN (SELECT MAX(id) FROM harness_items
                    WHERE kind = 'assistant' AND thread_id IN (${marks})
                    GROUP BY thread_id)`,
    ids,
  );
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

export async function getHarnessItems(threadId: number): Promise<HarnessItem[]> {
  const db = await getDb();
  return db.select<HarnessItem[]>(
    `SELECT * FROM harness_items WHERE thread_id = $1 ORDER BY id ASC`,
    [threadId],
  );
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
