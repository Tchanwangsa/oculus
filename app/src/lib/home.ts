import {
  getRecentlyAccessedFiles,
  getRecentlyWatchedLectures,
  type Lecture,
  type LibraryFileHit,
} from "@/lib/db";
import { sqliteUtcToMs } from "@/lib/format";
import { getHarnessThreads, type HarnessThread } from "@/lib/harness";

// ── Continue where you left off ──────────────────────────────────────────────

/** How far back "continue" reaches — older rows would look live but are not. */
const CONTINUE_MAX_AGE_MS = 14 * 24 * 60 * 60 * 1000;

/** One row of Home's Continue list. `at` is the raw stamp it was ranked by;
 *  convert with `sqliteUtcToMs` (see `stampMs`). */
export type ContinueItem =
  | { kind: "lecture"; at: string; lecture: Lecture & { subject_code: string } }
  | { kind: "file"; at: string; file: LibraryFileHit }
  | { kind: "thread"; at: string; thread: HarnessThread };

/** Every stamp goes through this: `datetime('now')` writes zoneless UTC,
 *  which `new Date` reads as local, while `harness_threads.updated_at` may
 *  be zoned ISO8601 — compared raw, the list sorts hours out of order. */
function stampMs(at: string | null): number | undefined {
  return sqliteUtcToMs(at);
}

/** Recent lectures, files and threads, merged newest first. Each source is
 *  fetched to `limit` since any one kind may fill the list. */
export async function loadContinue(limit = 4): Promise<ContinueItem[]> {
  const [lectures, files, threads] = await Promise.all([
    getRecentlyWatchedLectures(limit),
    getRecentlyAccessedFiles(limit),
    getHarnessThreads(),
  ]);

  const cutoff = Date.now() - CONTINUE_MAX_AGE_MS;
  const ranked: { ms: number; item: ContinueItem }[] = [];

  const push = (at: string | null, make: (at: string) => ContinueItem) => {
    if (!at) return;
    const ms = stampMs(at);
    if (ms == null || ms < cutoff) return;
    ranked.push({ ms, item: make(at) });
  };

  for (const lecture of lectures) {
    push(lecture.last_watched_at, (at) => ({ kind: "lecture", at, lecture }));
  }
  for (const file of files) {
    push(file.last_accessed_at, (at) => ({ kind: "file", at, file }));
  }
  for (const thread of threads) {
    // Untitled means no turn yet: nothing to continue.
    if (!thread.title?.trim()) continue;
    push(thread.updated_at, (at) => ({ kind: "thread", at, thread }));
  }

  ranked.sort((a, b) => b.ms - a.ms);
  return ranked.slice(0, limit).map((r) => r.item);
}
