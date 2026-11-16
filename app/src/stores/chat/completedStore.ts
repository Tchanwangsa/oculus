import { create } from "zustand";
import {
  getCompletedThreads,
  getSubjectCompletedCounts,
  getSubjectCompletedThreads,
  harnessCompleteThread,
  harnessReopenThread,
  type CompletedPage,
  type HarnessThread,
} from "@/lib/harness";

/** A Subject-view group's key: the subject's id, or "general" for none
 *  (the keys `groupBySubject` gives the column's groups). */
export const completedKey = (subjectId: number | null): string => (subjectId == null ? "general" : String(subjectId));

/** The rows a fold shows first; "Show more" adds `getCompletedThreads`' 25. */
export const COMPLETED_FIRST = 10;

interface SubjectFold {
  subjectId: number | null;
  page: CompletedPage;
}

interface CompletedState {
  /** Completed threads per group key; their sum is the Status view's count. */
  counts: Record<string, number>;
  /** The Status view's fold, every subject; null until it first opens. */
  all: CompletedPage | null;
  /** Subject folds read so far, by group key. */
  bySubject: Record<string, SubjectFold>;
  loadCounts: () => Promise<void>;
  /** The Status fold's first page. */
  loadAll: () => Promise<void>;
  moreAll: () => Promise<void>;
  loadSubject: (subjectId: number | null) => Promise<void>;
  moreSubject: (subjectId: number | null) => Promise<void>;
  /** Re-reads the counts and every fold read so far, each at the length it
   *  shows, after `harness-threads-changed`. */
  refresh: () => Promise<void>;
  complete: (threadId: number) => Promise<void>;
  reopen: (threadId: number) => Promise<void>;
}

/** `more` appended to `page`; a row a refresh already brought is not drawn twice. */
function append(page: CompletedPage, more: CompletedPage): CompletedPage {
  const seen = new Set(page.threads.map((t) => t.id));
  const threads: HarnessThread[] = [...page.threads, ...more.threads.filter((t) => !seen.has(t.id))];
  return { threads, total: more.total, nextOffset: more.nextOffset };
}

/** How many rows a refresh reads back: what the fold showed, at least a first page. */
const shownLength = (page: CompletedPage) => Math.max(COMPLETED_FIRST, page.threads.length);

/**
 * Completed threads for the thread column (`docs/harness.md`): the counts the
 * folds show closed, and the pages they show open. Rust completes and
 * reopens; this store reads, and re-reads when Rust says threads changed
 * (`watchThreadsChanged`).
 */
export const useCompletedStore = create<CompletedState>((set, get) => ({
  counts: {},
  all: null,
  bySubject: {},

  loadCounts: async () => {
    const rows = await getSubjectCompletedCounts();
    set({ counts: Object.fromEntries(rows.map((r) => [completedKey(r.subject_id), r.count])) });
  },

  loadAll: async () => {
    const all = await getCompletedThreads(0, COMPLETED_FIRST);
    set({ all });
  },

  moreAll: async () => {
    const at = get().all?.nextOffset;
    if (at == null) return;
    const more = await getCompletedThreads(at);
    set((s) => (s.all ? { all: append(s.all, more) } : s));
  },

  loadSubject: async (subjectId) => {
    const page = await getSubjectCompletedThreads(subjectId, 0, COMPLETED_FIRST);
    set((s) => ({ bySubject: { ...s.bySubject, [completedKey(subjectId)]: { subjectId, page } } }));
  },

  moreSubject: async (subjectId) => {
    const key = completedKey(subjectId);
    const at = get().bySubject[key]?.page.nextOffset;
    if (at == null) return;
    const more = await getSubjectCompletedThreads(subjectId, at);
    set((s) => {
      const fold = s.bySubject[key];
      return fold ? { bySubject: { ...s.bySubject, [key]: { subjectId, page: append(fold.page, more) } } } : s;
    });
  },

  refresh: async () => {
    const { all, bySubject } = get();
    const [counts, nextAll, folds] = await Promise.all([
      getSubjectCompletedCounts(),
      all ? getCompletedThreads(0, shownLength(all)) : null,
      Promise.all(
        Object.entries(bySubject).map(async ([key, f]) => {
          const page = await getSubjectCompletedThreads(f.subjectId, 0, shownLength(f.page));
          return [key, { subjectId: f.subjectId, page }] as const;
        }),
      ),
    ]);
    set({
      counts: Object.fromEntries(counts.map((r) => [completedKey(r.subject_id), r.count])),
      all: nextAll,
      bySubject: Object.fromEntries(folds),
    });
  },

  // Rust answers with `harness-threads-changed`, which re-reads both lists.
  complete: (threadId) => harnessCompleteThread(threadId),
  reopen: (threadId) => harnessReopenThread(threadId),
}));
