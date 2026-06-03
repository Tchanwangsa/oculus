import { create } from "zustand";

export type JobType =
  | "pdf_parse"
  | "canvas_sync"
  | "lecture_sync"
  | "lecture_download"
  | "transcript_download";

export type JobStatus = "queued" | "running" | "paused" | "completed" | "failed";

export interface Job {
  id: string;
  type: JobType;
  status: JobStatus;
  /** Short label shown after the type name, e.g. subject code or filename */
  label?: string;
  progress_current: number;
  progress_total: number;
  error?: string;
  /** Future: maps to jobs.id in SQLite for persist/resume */
  db_id?: string;
}

interface JobState {
  jobs: Record<string, Job>;
  upsert: (job: Job) => void;
  remove: (id: string) => void;
  clearCompleted: () => void;
}

export const useJobStore = create<JobState>((set) => ({
  jobs: {},

  upsert: (job) =>
    set((s) => ({ jobs: { ...s.jobs, [job.id]: job } })),

  remove: (id) =>
    set((s) => {
      const next = { ...s.jobs };
      delete next[id];
      return { jobs: next };
    }),

  clearCompleted: () =>
    set((s) => ({
      jobs: Object.fromEntries(
        Object.entries(s.jobs).filter(([, j]) => j.status !== "completed"),
      ),
    })),
}));
