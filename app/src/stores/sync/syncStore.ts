import { create } from "zustand";

export interface SyncProgress {
  done: number;
  total: number;
  course?: string;
  phase?: string;
  label?: string;
}

interface SyncSubjectRef {
  id: number;
  code: string;
}

interface SyncState {
  /** True from sync start until scrape-complete/error. */
  scraping: boolean;
  progress: SyncProgress | null;
  /** Active `sync_runs` row id. */
  runId: number | null;
  /** The active run's subjects, read at scrape-complete. */
  subjects: SyncSubjectRef[];
  /** Bumped on each completion; subscribe to reload. */
  completedAt: number;
  lastResult: { count: number; cancelled: boolean } | null;
  error: string | null;

  begin: (runId: number, subjects: SyncSubjectRef[]) => void;
  setProgress: (p: SyncProgress) => void;
  setRunId: (id: number | null) => void;
  complete: (count: number, cancelled: boolean) => void;
  fail: (msg: string) => void;
  reset: () => void;
}

export const useSyncStore = create<SyncState>((set) => ({
  scraping: false,
  progress: null,
  runId: null,
  subjects: [],
  completedAt: 0,
  lastResult: null,
  error: null,

  begin: (runId, subjects) =>
    set({ scraping: true, runId, subjects, progress: { done: 0, total: subjects.length }, error: null }),
  setProgress: (p) => set({ progress: p, scraping: true, error: null }),
  setRunId: (id) => set({ runId: id }),
  complete: (count, cancelled) =>
    set((s) => ({
      scraping: false, progress: null, runId: null, subjects: [],
      lastResult: { count, cancelled }, completedAt: s.completedAt + 1,
    })),
  fail: (msg) => set((s) => ({ scraping: false, runId: null, subjects: [], error: msg, completedAt: s.completedAt + 1 })),
  reset: () => set({ scraping: false, progress: null, error: null }),
}));
