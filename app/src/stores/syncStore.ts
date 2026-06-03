import { create } from "zustand";

export interface SyncProgress {
  done: number;
  total: number;
  course?: string;
  phase?: string;
  label?: string;
}

interface SyncState {
  /** True from sync start until scrape-complete/error. */
  scraping: boolean;
  progress: SyncProgress | null;
  /** Active sync_runs row id, for finishSyncRun bookkeeping. */
  runId: number | null;
  /** Monotonic counter bumped on each completion — UI subscribes to trigger reloads. */
  completedAt: number;
  lastResult: { count: number; cancelled: boolean } | null;
  error: string | null;

  // actions
  begin: (runId: number, total: number) => void;
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
  completedAt: 0,
  lastResult: null,
  error: null,

  begin: (runId, total) =>
    set({ scraping: true, runId, progress: { done: 0, total }, error: null }),
  setProgress: (p) => set({ progress: p, scraping: true, error: null }),
  setRunId: (id) => set({ runId: id }),
  complete: (count, cancelled) =>
    set((s) => ({
      scraping: false, progress: null, runId: null,
      lastResult: { count, cancelled }, completedAt: s.completedAt + 1,
    })),
  fail: (msg) => set((s) => ({ scraping: false, runId: null, error: msg, completedAt: s.completedAt + 1 })),
  reset: () => set({ scraping: false, progress: null, error: null }),
}));
