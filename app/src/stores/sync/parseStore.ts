import { create } from "zustand";

/** The `parse-status` event. `status`: queued | running | quality | error |
 *  skipped, where `"quality"` is success (frozen: every parsed row's
 *  `files.parse_status`) and `"skipped"` is the user's `parse_skip`. */
export interface ParseJob {
  relative_path: string;
  subject_id: number;
  status: string;
  /** "running": a cloud parse's sub-step; absent means processing. */
  phase?: "upload_wait" | "uploading" | "processing";
  /** "running": upload bytes, `bytes_total` from `upload_wait` on. */
  bytes_done?: number;
  bytes_total?: number;
  pages_done?: number;
  total_pages?: number;
  /** "queued": place in line, when it is known. */
  position?: number;
  /** "error": safe to display. */
  error?: string;
  kind?: string;
  /** "error": could retrying THIS file ever work? */
  retryable?: boolean;
  /** "error": does this condemn every other file too? */
  latching?: boolean;
}

/** A file's last failure; the background sweep reads it to know what not to
 *  re-kick. */
export interface ParseFailure {
  message: string;
  kind?: string;
  retryable?: boolean;
  latching?: boolean;
  at: number;
}

/** A failure that condemns every file (no token, rejected token, spent
 *  quota); the sweep backs off against it rather than repeat the error. */
export interface ParseLatch {
  message: string;
  kind?: string;
  at: number;
}

interface ParseState {
  /** Keyed by relative_path, as are `jobs` (queued/running only) and `failures`. */
  statuses: Record<string, string>;
  jobs: Record<string, ParseJob>;
  failures: Record<string, ParseFailure>;
  latch: ParseLatch | null;

  update: (ev: ParseJob) => void;
  /** Merge in disk-derived statuses without touching live jobs. */
  merge: (statuses: Record<string, string>) => void;
  /** Call on anything that plausibly fixed the latch, e.g. a token saved. */
  clearLatch: () => void;
}

export const useParseStore = create<ParseState>((set) => ({
  statuses: {},
  jobs: {},
  failures: {},
  latch: null,

  update: (ev) =>
    set((state) => {
      const statuses = { ...state.statuses, [ev.relative_path]: ev.status };
      const jobs = { ...state.jobs };
      if (ev.status === "queued" || ev.status === "running") {
        jobs[ev.relative_path] = ev;
      } else {
        delete jobs[ev.relative_path];
      }

      const failures = { ...state.failures };
      let latch = state.latch;
      if (ev.status === "error") {
        failures[ev.relative_path] = {
          message: ev.error ?? "Parse failed",
          kind: ev.kind,
          retryable: ev.retryable,
          latching: ev.latching,
          at: Date.now(),
        };
        if (ev.latching) {
          latch = { message: ev.error ?? "Parsing is unavailable", kind: ev.kind, at: Date.now() };
        }
      } else {
        // Any progress clears this file's failure.
        delete failures[ev.relative_path];
        // Only real parse work proves the latch lifted: `queued` precedes the
        // preflight, and a bare `quality` is an already-parsed skip. A cloud
        // parse can finish without a `running`, hence `quality` after `queued`.
        const parsed = ev.status === "quality" && state.jobs[ev.relative_path] != null;
        if (ev.status === "running" || parsed) latch = null;
      }

      return { statuses, jobs, failures, latch };
    }),

  merge: (incoming) =>
    set((state) => ({ statuses: { ...incoming, ...state.statuses } })),

  clearLatch: () => set({ latch: null }),
}));
