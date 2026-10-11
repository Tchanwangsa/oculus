import {
  getRecentlyAccessedFiles,
  getRecentlyWatchedLectures,
  type Lecture,
  type LibraryFileHit,
} from "@/lib/db";
import { sqliteUtcToMs } from "@/lib/format/format";
import { getHarnessThreads, type HarnessThread } from "@/lib/harness";

/** How far back Recent reaches — older rows would look live but are not. */
const RECENT_MAX_AGE_MS = 14 * 24 * 60 * 60 * 1000;

/** One card of Home's Recent row. `at` is the raw stamp it was ranked by;
 *  convert with `sqliteUtcToMs`. */
export type RecentItem =
  | { kind: "lecture"; at: string; lecture: Lecture & { subject_code: string } }
  | { kind: "file"; at: string; file: LibraryFileHit }
  | { kind: "thread"; at: string; thread: HarnessThread };

/** Recent lectures, files and threads, merged newest first. Each source is
 *  fetched to `limit` since any one kind may fill the row. */
export async function loadRecent(limit: number): Promise<RecentItem[]> {
  const [lectures, files, threads] = await Promise.all([
    getRecentlyWatchedLectures(limit),
    getRecentlyAccessedFiles(limit),
    getHarnessThreads(),
  ]);

  const cutoff = Date.now() - RECENT_MAX_AGE_MS;
  const ranked: { ms: number; item: RecentItem }[] = [];

  const push = (at: string | null, make: (at: string) => RecentItem) => {
    if (!at) return;
    const ms = sqliteUtcToMs(at);
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
    // Untitled means no turn yet: nothing to go back to.
    if (!thread.title?.trim()) continue;
    push(thread.updated_at, (at) => ({ kind: "thread", at, thread }));
  }

  ranked.sort((a, b) => b.ms - a.ms);
  return ranked.slice(0, limit).map((r) => r.item);
}
