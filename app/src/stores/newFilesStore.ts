import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { getDb } from "@/lib/db";
import { FILE_ACCESSED_EVENT } from "@/lib/openFile";

/**
 * Counts of never-opened files per subject and category, for the sidebar and
 * subject-tab badges. Only categories with openable rows count: anything else
 * could never be cleared.
 */
const COUNTED_CATEGORIES = ["file", "page", "announcement", "assignment", "quiz", "ed"];

/** Keyed by the tab's path segment (`TABS` in `SubjectLayout`). */
export const TAB_CATEGORIES: Record<string, string[]> = {
  modules: ["page"],
  files: ["file"],
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

/** Keeps the counts current on file opens and syncs. Called once at the root. */
export function watchNewFiles(): () => void {
  const refresh = () => void useNewFilesStore.getState().refresh();

  refresh();
  window.addEventListener(FILE_ACCESSED_EVENT, refresh);

  let timer: ReturnType<typeof setTimeout> | undefined;
  const debounced = () => {
    clearTimeout(timer);
    // Lets the other listener's DB upsert for this event land before counting.
    timer = setTimeout(refresh, 1500);
  };
  const unsubs = [listen("scrape-file", debounced), listen("scrape-complete", debounced)];

  return () => {
    window.removeEventListener(FILE_ACCESSED_EVENT, refresh);
    clearTimeout(timer);
    unsubs.forEach((u) => u.then((f) => f()).catch(() => {}));
  };
}
