import { create } from "zustand";

/**
 * The `parse-status` event, exactly as Rust emits it.
 *
 * `status` is `"queued" | "running" | "quality" | "error"`. **`"quality"` is
 * the terminal success**, and the name outlived the tier it was named after:
 * there is only one parse now, but `files.parse_status = 'quality'` is what
 * every already-parsed row in the library says and what Rust's "already done"
 * check reads, so the string is frozen.
 */
export interface ParseJob {
  relative_path: string;
  subject_id: number;
  status: string;
  /** "running". */
  pages_done?: number;
  total_pages?: number;
  /** "queued": place in line, when it is known. */
  position?: number;
  /** "error": human-readable, safe to display. */
  error?: string;
  /** "error": machine-readable discriminant. */
  kind?: string;
  /** "error": could retrying THIS file ever work? */
  retryable?: boolean;
  /** "error": does this condemn every other file too? */
  latching?: boolean;
}

/** The discriminants of a file's last failure, kept so the failure UI can say
 *  what went wrong and so the background sweep knows what not to re-kick. */
export interface ParseFailure {
  message: string;
  kind?: string;
  retryable?: boolean;
  latching?: boolean;
  at: number;
}

/** A failure that condemns the whole library rather than one file — no token,
 *  a rejected token, exhausted quota. Every parse now costs metered cloud
 *  quota, so this is what the sweep backs off against instead of marching
 *  every remaining file into the same error. */
export interface ParseLatch {
  message: string;
  kind?: string;
  at: number;
}

interface ParseState {
  /** relative_path -> latest status string (for file-row badges). */
  statuses: Record<string, string>;
  /** relative_path -> active job (only queued/running kept). */
  jobs: Record<string, ParseJob>;
  /** relative_path -> why its last attempt failed. Cleared when it moves again. */
  failures: Record<string, ParseFailure>;
  /** The latching condition in force, if any. */
  latch: ParseLatch | null;

  update: (ev: ParseJob) => void;
  /** Merge in disk-derived statuses without touching live jobs. */
  merge: (statuses: Record<string, string>) => void;
  /** Lift the latch — anything that plausibly fixed the condition (a token
   *  saved, a parse that then succeeded) should call this. */
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
        // The file is moving, so whatever it last failed with is history — and
        // a parse getting anywhere at all is proof the latching condition (if
        // there was one) has lifted.
        delete failures[ev.relative_path];
        latch = null;
      }

      return { statuses, jobs, failures, latch };
    }),

  merge: (incoming) =>
    set((state) => ({ statuses: { ...incoming, ...state.statuses } })),

  clearLatch: () => set({ latch: null }),
}));
