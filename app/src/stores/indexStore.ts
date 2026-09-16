import { create } from "zustand";

import { embedPending } from "@/lib/retrieval";

/**
 * The index run — the app's own way to embed what is outstanding.
 *
 * Until this existed the only way to build the index was `oculus index` from a
 * terminal: `embedFile` and `embedPending` were both written and wired to Rust
 * and neither had a caller, so a student with 166 parsed PDFs and no searchable
 * pages had nothing in the UI to press. That is what this store is for.
 *
 * **It is one run at a time, deliberately.** `embedPending` is serial because
 * the cloud backend paces itself against the account's per-minute ceiling, so a
 * second concurrent run would not go faster — it would split the same tokens
 * per minute between two loops and make both look stalled. `start` is a no-op
 * while one is running rather than a queue.
 *
 * **It can legitimately run for hours.** On a Voyage account with no payment
 * method the ceiling is ~2.8 pages a minute, so the whole library is most of a
 * day. That is why there is a `stop`, why progress names the file it is on
 * rather than only a percentage, and why the run survives navigation: the state
 * lives here rather than in the settings page's component.
 *
 * Stopping is cooperative and lands **between files**, never mid-document —
 * `embedPending` checks it before each one. A half-embedded PDF is not a
 * corrupt state (the next run re-embeds it, since `is_embedded` compares page
 * coverage), but abandoning a document mid-flight would waste the quota it had
 * already spent.
 */
export interface IndexProgress {
  done: number;
  total: number;
  /** The file being embedded now; empty on the final callback. */
  filename: string;
}

export interface IndexResult {
  files: number;
  pages: number;
  errors: string[];
  stopped: boolean;
}

export interface IndexState {
  running: boolean;
  progress: IndexProgress | null;
  /** Set while a stop has been asked for but the current file has not ended. */
  stopping: boolean;
  /** The last finished run, kept so the page can report it after the fact. */
  result: IndexResult | null;
  error: string | null;

  start: () => Promise<void>;
  stop: () => void;
}

export const useIndexStore = create<IndexState>((set, get) => ({
  running: false,
  progress: null,
  stopping: false,
  result: null,
  error: null,

  start: async () => {
    if (get().running) return;
    set({ running: true, stopping: false, progress: null, result: null, error: null });
    try {
      const outcome = await embedPending(
        undefined,
        (done, total, filename) => set({ progress: { done, total, filename } }),
        () => get().stopping,
      );
      set({ result: { ...outcome }, progress: null });
    } catch (cause) {
      console.error("index run failed", cause);
      set({ error: String(cause), progress: null });
    } finally {
      set({ running: false, stopping: false });
    }
  },

  // Only a flag: the loop owns when it acts on it, so the run always ends on a
  // file boundary with its record and page rows written.
  stop: () => {
    if (get().running) set({ stopping: true });
  },
}));
