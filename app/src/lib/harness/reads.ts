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

/** A thread's `run_started_at`: when the run in flight started (asked, if
 *  it has not started yet), for the list's Working timer; null when none is. */
const RUN_STARTED_AT =
  "(SELECT MIN(COALESCE(r.started_at, r.requested_at)) FROM harness_runs r WHERE r.thread_id = t.id" +
  " AND r.execution_status IN ('preparing', 'starting', 'running')) AS run_started_at";

const THREAD_COLUMNS = `t.*, ${SETTLED_AT}, ${PENDING_REQUESTS}, ${PENDING_QUESTIONS}, ${RUN_STARTED_AT}`;

/** The thread list's read, newest first; `$1` is the limit. */
export const THREADS_SQL = `SELECT ${THREAD_COLUMNS} FROM harness_threads t ORDER BY t.updated_at DESC, t.id DESC LIMIT $1`;

export async function getHarnessThreads(limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(THREADS_SQL, [limit]);
}

/** One lecture's threads — its own query, since `getHarnessThreads` caps
 *  library-wide and would drop a lecture's older ones. */
export async function getLectureThreads(lectureId: string, limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(
    `SELECT ${THREAD_COLUMNS} FROM harness_threads t WHERE t.lecture_id = $1
      ORDER BY t.updated_at DESC, t.id DESC LIMIT $2`,
    [lectureId, limit],
  );
}

const marks = (count: number) => Array.from({ length: count }, (_, i) => `$${i + 1}`).join(", ");

/** `getThreadPreviews`' read for `count` thread ids: each thread's newest
 *  message from either side. An agent message is a run's selected answer, or
 *  a reply with no run (history from before runs, a turn the provider started
 *  itself), as thread naming reads it, so a run with none (one that waits, or
 *  was cancelled) adds nothing. Item ids rise with time: the larger is newer. */
export function previewsSql(count: number): string {
  return `SELECT thread_id, content FROM harness_items
      WHERE id IN (SELECT MAX(i.id) FROM harness_items i
                     LEFT JOIN harness_runs r ON r.id = i.run_id
                    WHERE (i.kind = 'user' OR (i.kind = 'assistant' AND (i.run_id IS NULL OR r.answer_item_id = i.id)))
                      AND i.thread_id IN (${marks(count)})
                    GROUP BY i.thread_id)`;
}

/** `getPendingQuestions`' read: the first question of each thread's newest
 *  question waiting on the student (the payload `requests/questions.rs` writes). */
export function questionsSql(count: number): string {
  return `SELECT thread_id, json_extract(payload, '$.questions[0].text') AS content FROM harness_requests
      WHERE id IN (SELECT MAX(q.id) FROM harness_requests q
                    WHERE q.kind = 'question' AND q.status = 'pending' AND q.responder IN ('live', 'message')
                      AND q.thread_id IN (${marks(count)})
                    GROUP BY q.thread_id)`;
}

/** Markdown off and whitespace folded, so a message reads as one line. */
export function flattenPreview(content: string | null): string {
  return (content ?? "")
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^\s*(#+|>+|[-*+]|\d+\.)\s+/gm, "")
    // Not `_`: it is inside paths and course codes far more than emphasis.
    .replace(/[*`~|$]+/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

async function readLines(sql: string, ids: number[]): Promise<Map<number, string>> {
  const out = new Map<number, string>();
  if (!ids.length) return out;
  const db = await getDb();
  const rows = await db.select<{ thread_id: number; content: string | null }[]>(sql, ids);
  for (const r of rows) {
    const text = flattenPreview(r.content);
    if (text) out.set(r.thread_id, text);
  }
  return out;
}

/** Each thread's newest message, either side's, as one line of plain text;
 *  threads with none yet are absent. */
export function getThreadPreviews(ids: number[]): Promise<Map<number, string>> {
  return readLines(previewsSql(ids.length), ids);
}

/** Each thread's waiting question as one line; threads asking none are absent. */
export function getPendingQuestions(ids: number[]): Promise<Map<number, string>> {
  return readLines(questionsSql(ids.length), ids);
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
