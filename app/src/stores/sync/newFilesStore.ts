import { create } from "zustand";
import { shallow } from "zustand/shallow";
import { listen } from "@tauri-apps/api/event";
import { getDb } from "@/lib/db";
import { FILE_ACCESSED_EVENT } from "@/lib/files/openFile";
import { COURSE_DIR, COURSES_DIR, LESSONS_DIR } from "@/lib/files/libraryLayout";

/**
 * Counts of never-opened synced files per subject and category, for the
 * sidebar's Subjects dot and subject-tab badges. Only files inside `Course/`
 * count (the student's own files are theirs, never news), and only
 * categories with openable rows: anything else could never be cleared. Of
 * `lessons`, only the lesson `.md`s the Lessons tab lists count, not the
 * assets in their folders.
 */
const COUNTED_CATEGORIES = ["file", "page", "announcement", "assignment", "quiz", "ed", "lessons"];

/** The tabs that list one kind, keyed by the tab's path segment (`TABS` in
 *  `SubjectNav`). Files lists the whole of `Course/`, so its count is all
 *  of them (`ALL_SYNCED`). */
const TAB_CATEGORIES: Record<string, string[]> = {
  modules: ["page"],
  announcements: ["announcement"],
  assignments: ["assignment", "quiz"],
  discussion: ["ed"],
  lessons: ["lessons"],
};
const ALL_SYNCED = "files";

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
           AND relative_path GLOB '${COURSES_DIR}/*/${COURSE_DIR}/*'
           AND NOT (category = 'lessons' AND relative_path GLOB '${COURSES_DIR}/*/${LESSONS_DIR}/*/*')
         GROUP BY subject_id, category`,
      );
      const bySubject: CountsBySubject = {};
      for (const r of rows) {
        (bySubject[r.subject_id] ??= {})[r.category] = r.n;
      }
      set((s) => {
        for (const key of Object.keys(bySubject)) {
          const id = Number(key);
          if (shallow(s.bySubject[id], bySubject[id])) bySubject[id] = s.bySubject[id];
        }
        return shallow(s.bySubject, bySubject) ? s : { bySubject };
      });
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
  if (tab === ALL_SYNCED) return newCountForSubject(counts, subjectId);
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
