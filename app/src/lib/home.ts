import {
  getRecentlyAccessedFiles,
  getRecentlyWatchedLectures,
  type Lecture,
  type LibraryFileHit,
} from "@/lib/db";
import { sqliteUtcToMs } from "@/lib/format";
import { getHarnessThreads, type HarnessThread } from "@/lib/harness";

// ── Continue where you left off ──────────────────────────────────────────────

/**
 * How far back "continue" reaches. Fourteen days: a file you opened in March
 * is not something you are in the middle of, and without a floor the section
 * would show the same three rows forever on a library nobody has touched in a
 * month — which is worse than showing nothing, because it looks live.
 */
export const CONTINUE_MAX_AGE_MS = 14 * 24 * 60 * 60 * 1000;

/**
 * One row of Home's Continue list. A discriminated union rather than a
 * flattened row because the three kinds open in three different ways — the UI
 * needs the whole lecture / file / thread, not a lowest common denominator of
 * them.
 *
 * `at` is the stamp the entry was *ranked* by, carried along so the UI can say
 * "2 hours ago" without knowing which column it came from. It is a raw SQLite
 * string: put it through `sqliteUtcToMs` before doing anything with it (see
 * the hazard note on `stampMs` below).
 */
export type ContinueItem =
  | { kind: "lecture"; at: string; lecture: Lecture & { subject_code: string } }
  | { kind: "file"; at: string; file: LibraryFileHit }
  | { kind: "thread"; at: string; thread: HarnessThread };

/**
 * Epoch ms for any of the three stamps — the one conversion every comparison
 * in this file goes through.
 *
 * **The hazard this exists for**: the three columns are written by different
 * code with different idioms. `files.last_accessed_at` and
 * `lectures.last_watched_at` come from SQLite's `datetime('now')` — UTC,
 * `"YYYY-MM-DD HH:MM:SS"`, with no zone marker, which `new Date(...)` reads as
 * *local* time; `harness_threads.updated_at` may be that or an ISO8601 stamp
 * that already carries its zone. Compare one against the other raw and the
 * list silently sorts hours out of order — no error, just a wrong answer.
 * `sqliteUtcToMs` handles both shapes, so every stamp goes through it,
 * including the ones that are obviously SQLite's.
 */
function stampMs(at: string | null): number | undefined {
  return sqliteUtcToMs(at);
}

/**
 * The Continue list: the lectures, files and conversations you were last in,
 * merged and ranked by their own recency stamps, newest first.
 *
 * There is no store behind this — the section reads on mount, on the front
 * edge of its tab, and on `FILE_ACCESSED_EVENT` / `LECTURES_CHANGED_EVENT`
 * (`useHomeSection` in `app/src/components/home/`), the way the other pages
 * do. Playback's `LECTURE_PROGRESS_EVENT` is deliberately not one of them: it
 * fires every five seconds of a recording, and re-ranking this list that often
 * for a row nobody is looking at is three queries a tick. Each source is
 * over-fetched to `limit` and the merge does the capping, since any one kind
 * can legitimately fill the whole list.
 */
export async function loadContinue(limit = 4): Promise<ContinueItem[]> {
  const [lectures, files, threads] = await Promise.all([
    getRecentlyWatchedLectures(limit),
    getRecentlyAccessedFiles(limit),
    // Already `ORDER BY updated_at DESC` — reuse it rather than writing a
    // second query over the same table.
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
    // An untitled thread is one nobody has said anything in yet — the title
    // arrives with the first turn — so there is nothing to continue.
    if (!thread.title?.trim()) continue;
    push(thread.updated_at, (at) => ({ kind: "thread", at, thread }));
  }

  ranked.sort((a, b) => b.ms - a.ms);
  return ranked.slice(0, limit).map((r) => r.item);
}

// Relative-time labels ("12m ago", "3h ago") are `fmtAgo` in
// `app/src/lib/format.ts`, which already takes epoch ms and shares its buckets
// with `fmtDate`. Pair it with `sqliteUtcToMs(item.at)`; there is deliberately
// no second copy here.
