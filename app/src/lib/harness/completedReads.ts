import { getDb } from "@/lib/db";
import type { HarnessThread } from "./types";

export interface CompletedPage {
  threads: HarnessThread[];
  total: number;
  nextOffset: number | null;
}

export interface SubjectCompletedCount {
  subject_id: number | null;
  count: number;
}

export const COMPLETED_COUNTS_SQL = `SELECT subject_id, COUNT(*) AS count FROM harness_threads
  WHERE completed_at IS NOT NULL GROUP BY subject_id`;

/** Null selects General; undefined selects all subjects. */
export function completedSql(subjectId?: number | null): { rows: string; count: string } {
  const where = `FROM harness_threads t WHERE t.completed_at IS NOT NULL${subjectId === undefined ? "" : " AND t.subject_id IS $3"}`;
  return {
    rows: `SELECT t.* ${where} ORDER BY julianday(t.updated_at) DESC, t.id DESC LIMIT $1 OFFSET $2`,
    count: `SELECT COUNT(*) AS count ${where.replace("$3", "$1")}`,
  };
}

async function readPage(offset: number, limit: number, subjectId?: number | null): Promise<CompletedPage> {
  const db = await getDb();
  const sql = completedSql(subjectId);
  const subject = subjectId === undefined ? [] : [subjectId];
  const threads = await db.select<HarnessThread[]>(sql.rows, [limit, offset, ...subject]);
  const [count] = await db.select<{ count: number }[]>(sql.count, subject);
  const total = count.count;
  const next = offset + threads.length;
  return { threads, total, nextOffset: threads.length > 0 && next < total ? next : null };
}

/** Initial fold: 10 rows. Subsequent pages default to 25. */
export function getCompletedThreads(offset = 0, limit = offset === 0 ? 10 : 25): Promise<CompletedPage> {
  return readPage(offset, limit);
}

export function getSubjectCompletedThreads(subjectId: number | null, offset = 0, limit = offset === 0 ? 10 : 25): Promise<CompletedPage> {
  return readPage(offset, limit, subjectId);
}

export async function getSubjectCompletedCounts(): Promise<SubjectCompletedCount[]> {
  const db = await getDb();
  return db.select<SubjectCompletedCount[]>(COMPLETED_COUNTS_SQL);
}
