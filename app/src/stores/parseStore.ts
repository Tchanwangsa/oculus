import { create } from "zustand";

export interface ParseJob {
  relative_path: string;
  subject_id: number;
  status: string; // "fast" | "queued" | "running" | "quality" | "error"
  pages_done?: number;
  total_pages?: number;
  /** For "queued": place in the sidecar's single-slot quality queue. */
  position?: number;
  error?: string;
}

interface ParseState {
  /** relative_path -> latest status string (for file-row badges). */
  statuses: Record<string, string>;
  /** relative_path -> active job (only queued/running kept, for the aggregate toast). */
  jobs: Record<string, ParseJob>;

  update: (ev: ParseJob) => void;
  /** Merge in disk-derived statuses without touching live jobs. */
  merge: (statuses: Record<string, string>) => void;
}

export const useParseStore = create<ParseState>((set) => ({
  statuses: {},
  jobs: {},

  update: (ev) =>
    set((state) => {
      const statuses = { ...state.statuses, [ev.relative_path]: ev.status };
      const jobs = { ...state.jobs };
      if (ev.status === "queued" || ev.status === "running") {
        jobs[ev.relative_path] = ev;
      } else {
        delete jobs[ev.relative_path];
      }
      return { statuses, jobs };
    }),

  merge: (incoming) =>
    set((state) => ({ statuses: { ...incoming, ...state.statuses } })),
}));
