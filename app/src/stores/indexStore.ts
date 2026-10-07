import { create } from "zustand";

import {
  clearEmbedQueued,
  getQueuedEmbedRows,
  markEmbedQueued,
  type DbFile,
} from "@/lib/db";
import {
  embedBlocked,
  embedFile,
  embeddingStats,
  embedReady,
  getUnembeddedPdfs,
} from "@/lib/retrieval";
import { usePipelineStore } from "@/stores/pipelineStore";

/**
 * The index run: a queue of files to embed, fed by the Index button in
 * Settings → Embeddings (the whole backlog) and, once `ready`, by each finished
 * parse (`useBackendEvents`). One serial worker drains it, because Voyage
 * paces against a per-minute ceiling and a second loop would only split it.
 * A run can take hours, so it lives here (surviving navigation) and has a
 * cooperative `stop` that lands between files.
 *
 * Nothing is queued until a Voyage key is stored (`ready`), and saving a key
 * does not sweep up the backlog: that is metered work the settings page's
 * estimate is meant to be read before.
 *
 * The queue is persisted as `files.embed_status = 'queued'`: set on enqueue,
 * cleared when a job leaves unrun or once it has run, and left in place by a
 * quit, so `restoreIndexQueue` can pick the approved work back up at boot.
 */

export interface IndexProgress {
  /** Files finished / in this run; `total` grows as parses land. */
  done: number;
  total: number;
  /** The file being embedded now; empty on the final callback. */
  filename: string;
  /** Pages inside that file, from `embed-status`; zero until its first batch
   *  returns, so the UI must not draw 0/0 as a full bar. */
  pagesDone: number;
  totalPages: number;
}

interface IndexResult {
  files: number;
  pages: number;
  errors: string[];
  stopped: boolean;
}

interface IndexJob {
  fileId: number;
  subjectId: number;
  relativePath: string;
  filename: string;
}

export interface IndexState {
  running: boolean;
  progress: IndexProgress | null;
  /** A stop was asked for; the current file has not ended yet. */
  stopping: boolean;
  /** The last finished run. */
  result: IndexResult | null;
  error: string | null;
  /** A Voyage key is stored: the gate in front of auto-embed. */
  ready: boolean;

  /** `false` also stops the run. */
  setReady: (ready: boolean) => void;
  /** Append files (skipping queued and in-flight paths); start the worker if
   *  idle. Refused while a stop is pending. */
  enqueue: (jobs: IndexJob[]) => void;
  /** The pipeline table's retry. */
  enqueueFile: (file: DbFile) => void;
  /** The Index button: queue the whole backlog. */
  start: () => Promise<void>;
  stop: () => void;
}

/** Outside the store on purpose: Zustand replaces state, so a worker holding
 *  a snapshot would drain a copy and miss files appended mid-run. */
let queue: IndexJob[] = [];
let inFlight: string | null = null;

/** The DB marks, written in order so a clear never lands before the mark it
 *  undoes; the worker waits on it so Rust's `done` cannot be overwritten. */
let marks: Promise<void> = Promise.resolve();

function persist(write: () => Promise<void>): void {
  marks = marks.then(write).catch((cause) => console.error("embed queue mark failed", cause));
}

function queuedPaths(): Set<string> {
  const paths = new Set(queue.map((job) => job.relativePath));
  if (inFlight) paths.add(inFlight);
  return paths;
}

/** Rust's `embed-status: queued` fires only when the worker reaches a file,
 *  so the pipeline table learns about the rest of the queue from here. */
function markQueued(jobs: IndexJob[]): void {
  const { touch } = usePipelineStore.getState();
  for (const job of jobs) touch(job.relativePath, job.subjectId, { embed: "queued" });
}

/** Empty the queue, handing its never-run jobs back to the table as pending. */
function dropQueue(): void {
  const dropped = queue;
  queue = [];
  const ids = dropped.map((job) => job.fileId);
  persist(() => clearEmbedQueued(ids));
  const { items, touch } = usePipelineStore.getState();
  for (const job of dropped) {
    if (items[job.relativePath]?.embed === "queued") {
      touch(job.relativePath, job.subjectId, { embed: "pending" });
    }
  }
}

export const useIndexStore = create<IndexState>((set, get) => ({
  running: false,
  progress: null,
  stopping: false,
  result: null,
  error: null,
  ready: false,

  setReady: (ready) => {
    if (get().ready === ready) return;
    set({ ready });
    // This store owns whether the pipeline table's embed stage applies.
    usePipelineStore.getState().setEmbedStage(ready);
    // Without a key every remaining job would fail.
    if (!ready) get().stop();
  },

  enqueue: (jobs) => {
    // A stop empties the queue for good: work arriving before it lands is
    // refused (rows stay pending) rather than outliving the stop.
    if (get().stopping) return;
    // A path already waiting is marked again: a re-parse resets its status.
    const ids = jobs.filter((job) => job.relativePath !== inFlight).map((job) => job.fileId);
    persist(() => markEmbedQueued(ids));
    const seen = queuedPaths();
    const fresh = jobs.filter((job) => !seen.has(job.relativePath));
    if (fresh.length === 0) return;
    queue.push(...fresh);
    markQueued(fresh);

    const state = get();
    const total = (state.progress?.total ?? 0) + fresh.length;
    if (state.running) {
      set({ progress: { ...(state.progress ?? blankProgress()), total } });
      return;
    }
    set({
      running: true,
      stopping: false,
      result: null,
      error: null,
      progress: { ...blankProgress(), total },
    });
    void drain();
  },

  enqueueFile: (file) => {
    get().enqueue([
      {
        fileId: file.id,
        subjectId: file.subject_id,
        relativePath: file.relative_path,
        filename: file.filename,
      },
    ]);
  },

  start: async () => {
    try {
      const pending = await getUnembeddedPdfs();
      get().enqueue(
        pending.map((f) => ({
          fileId: f.id,
          subjectId: f.subject_id,
          relativePath: f.relative_path,
          filename: f.filename,
        })),
      );
    } catch (cause) {
      console.error("index run failed to start", cause);
      set({ error: String(cause) });
    }
  },

  // The worker acts on the flag between files, never mid-document.
  stop: () => {
    if (!get().running) return;
    dropQueue();
    set({ stopping: true });
  },
}));

function blankProgress(): IndexProgress {
  return { done: 0, total: 0, filename: "", pagesDone: 0, totalPages: 0 };
}

/** Page progress from `embed-status`, for the in-flight file only: the event
 *  also fires for one-off retries outside the run. */
export function reportEmbedPages(
  relativePath: string,
  pagesDone: number,
  totalPages: number,
): void {
  if (relativePath !== inFlight) return;
  const progress = useIndexStore.getState().progress;
  if (!progress) return;
  useIndexStore.setState({ progress: { ...progress, pagesDone, totalPages } });
}

/**
 * The worker. A per-file failure does not end the run; an account-wide one (a
 * spent allowance, the spend limit) does, since every queued file would fail
 * the same way. `embedBlocked` (local, sends nothing) is asked only after a
 * failure.
 */
async function drain(): Promise<void> {
  const errors: string[] = [];
  let pages = 0;
  let done = 0;
  let stopped = false;

  try {
    for (;;) {
      if (useIndexStore.getState().stopping) {
        stopped = true;
        break;
      }
      const job = queue.shift();
      if (!job) break;

      inFlight = job.relativePath;
      const progress = useIndexStore.getState().progress ?? blankProgress();
      useIndexStore.setState({
        progress: { ...progress, done, filename: job.filename, pagesDone: 0, totalPages: 0 },
      });

      let blocked = false;
      try {
        await marks;
        const summary = await embedFile(job.fileId, job.subjectId, job.relativePath);
        pages += summary.pages_embedded;
      } catch (e) {
        errors.push(`${job.filename}: ${e}`);
        blocked = (await embedBlocked().catch(() => null)) != null;
      } finally {
        // Its `done`/`error` comes from the event; this catches a run that
        // ended without one, so it is not restored next session.
        persist(() => clearEmbedQueued([job.fileId]));
        inFlight = null;
      }
      done += 1;
      if (blocked) {
        dropQueue();
        stopped = true;
        break;
      }
    }
  } catch (cause) {
    console.error("index run failed", cause);
    useIndexStore.setState({ error: String(cause) });
  } finally {
    inFlight = null;
    // Nothing outlives the worker; a thrown run leaves jobs here otherwise.
    dropQueue();
    const total = useIndexStore.getState().progress?.total ?? done;
    useIndexStore.setState({
      running: false,
      stopping: false,
      progress: null,
      result: { files: done - errors.length, pages, errors, stopped: stopped || done < total },
    });
  }
}

let restored = false;

/**
 * Re-enqueue what the last session left marked `queued`. Called once at boot
 * (`App.tsx`), never from Settings: saving a key there must not start metered
 * work, so a session that boots without a usable key restores nothing.
 * A mark on a row that is no longer a parsed PDF, or that is already fully
 * embedded in the current space, is cleared instead.
 */
export async function restoreIndexQueue(): Promise<void> {
  if (restored) return;
  restored = true;
  try {
    if (!(await embedReady())) return;
    const { model, dim } = await embeddingStats();
    const rows = await getQueuedEmbedRows(model, dim);
    const stale = rows.filter(
      (r) => !r.embeddable || r.pages_current >= Math.max(r.pages_total, 1),
    );
    if (stale.length > 0) persist(() => clearEmbedQueued(stale.map((r) => r.id)));
    const live = rows.filter((r) => !stale.includes(r));
    useIndexStore.getState().enqueue(
      live.map((r) => ({
        fileId: r.id,
        subjectId: r.subject_id,
        relativePath: r.relative_path,
        filename: r.filename,
      })),
    );
  } catch (cause) {
    console.error("embed queue restore failed", cause);
  }
}
