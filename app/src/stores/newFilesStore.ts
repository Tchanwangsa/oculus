import { create } from "zustand";
import { getDb } from "@/lib/db";
import { FILE_ACCESSED_EVENT } from "@/lib/openFile";
import { SCRAPED_FILE_SAVED_EVENT } from "@/lib/syncWrites";

/**
 * Counts of "new" files — scraped since recency tracking began and never
 * opened — per subject and category. Drives the notification badges on the
 * sidebar subject rows and the subject tab strip.
 *
 * Only categories that render as openable rows count: anything else (module
 * TOCs, inline images, home/syllabus docs) has no row whose opening would
 * clear it, so a badge fed by it could never be dismissed.
 */
const COUNTED_CATEGORIES = ["file", "page", "announcement", "assignment", "quiz", "ed"];

/** Which categories each subject tab surfaces. */
export const TAB_CATEGORIES: Record<string, string[]> = {
  modules: ["page"],
  downloads: ["file"],
  announcements: ["announcement"],
  assignments: ["assignment", "quiz"],
  discussion: ["ed"],
};

type CountsBySubject = Record<number, Record<string, number>>;

interface NewFilesState {
  bySubject: CountsBySubject;
  refresh: () => Promise<void>;
}

export const useNewFilesStore = create<NewFilesState>((set) => ({
  bySubject: {},

  refresh: async () => {
    try {
      const db = await getDb();
      const rows = await db.select<{ subject_id: number; category: string; n: number }[]>(
        `SELECT subject_id, category, COUNT(*) AS n
         FROM files
         WHERE first_seen_at IS NOT NULL AND last_accessed_at IS NULL
           AND category IN (${COUNTED_CATEGORIES.map((c) => `'${c}'`).join(", ")})
         GROUP BY subject_id, category`,
      );
      const bySubject: CountsBySubject = {};
      for (const r of rows) {
        (bySubject[r.subject_id] ??= {})[r.category] = r.n;
      }
      set({ bySubject });
    } catch {
      /* db not ready yet — the next trigger retries */
    }
  },
}));

export function newCountForSubject(counts: CountsBySubject, subjectId: number): number {
  const cats = counts[subjectId];
  return cats ? Object.values(cats).reduce((a, b) => a + b, 0) : 0;
}

export function newCountForTab(
  counts: CountsBySubject,
  subjectId: number,
  tab: string,
): number {
  const cats = counts[subjectId];
  const wanted = TAB_CATEGORIES[tab];
  if (!cats || !wanted) return 0;
  return wanted.reduce((sum, c) => sum + (cats[c] ?? 0), 0);
}

/**
 * Keep the counts current: opening a file clears its dot, and a running sync
 * adds new ones. Called once from the app root (alongside useBackendEvents).
 * Committed file events arrive per file, so those refreshes are debounced.
 */
export function watchNewFiles(): () => void {
  const refresh = () => void useNewFilesStore.getState().refresh();

  refresh();
  window.addEventListener(FILE_ACCESSED_EVENT, refresh);

  let timer: ReturnType<typeof setTimeout> | undefined;
  const debounced = () => {
    clearTimeout(timer);
    // This event is emitted after the metadata commit. Debouncing coalesces
    // large batches; it no longer guesses how long the write queue will take.
    timer = setTimeout(refresh, 1500);
  };
  window.addEventListener(SCRAPED_FILE_SAVED_EVENT, debounced);

  return () => {
    window.removeEventListener(FILE_ACCESSED_EVENT, refresh);
    clearTimeout(timer);
    window.removeEventListener(SCRAPED_FILE_SAVED_EVENT, debounced);
  };
}
