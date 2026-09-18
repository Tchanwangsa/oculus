import { create } from "zustand";

import { type DbFile } from "@/lib/db";
import { embedBlocked, embedFile, getUnembeddedPdfs } from "@/lib/retrieval";
import { usePipelineStore } from "@/stores/pipelineStore";

/**
 * The index run — the app's own way to embed what is outstanding.
 *
 * Until this existed the only way to build the index was `oculus index` from a
 * terminal: `embedFile` was written and wired to Rust and had no caller, so a
 * student with 166 parsed PDFs and no searchable pages had nothing in the UI
 * to press. It is now **a queue rather than a batch**, and that is the one
 * structural change: work arrives from two places — the Index button in
 * Settings → Library, which fills it with the whole backlog, and a parse
 * finishing, which appends one file. Both feed the same serial worker, so
 * there is never a second loop splitting the same tokens per minute.
 *
 * **It is one file at a time, deliberately.** The cloud backend paces itself
 * against the account's per-minute ceiling, so a second concurrent run would
 * not go faster — it would split the same tokens between two loops and make
 * both look stalled.
 *
 * **It can legitimately run for hours.** On a Voyage account with no payment
 * method the ceiling is ~2.8 pages a minute, so the whole library is most of a
 * day. That is why there is a `stop`, why progress names the file it is on and
 * the page inside it rather than only a percentage, and why the run survives
 * navigation: the state lives here rather than in the settings page's
 * component.
 *
 * Stopping is cooperative and lands **between files**, never mid-document —
 * the worker checks before each one. A half-embedded PDF is not a corrupt
 * state (the next run re-embeds it, since `is_embedded` compares page
 * coverage), but abandoning a document mid-flight would waste the quota it had
 * already spent.
 *
 * ## Auto-embed, and the switch in front of it
 *
 * `ready` is whether a Voyage key is stored. Nothing is queued automatically
 * until it is — an app that started embedding before it had somewhere to embed
 * to would produce one failed row per parsed file and teach the user that the
 * third stage is broken. Once it is true, a finished parse appends its file
 * here (`useBackendEvents`), so a sync runs download → parse → embed end to
 * end. The brakes are the ones that were already there: the spend guard in
 * Settings → Library, the account-wide latch below, and Stop.
 *
 * The **backlog is not** swept up automatically when a key is saved. Draining
 * 166 files is hours of metered work and there is an estimate on the settings
 * page written to be read before it starts; auto-embed covers what arrives
 * from now on, the Index button covers what was already there.
 */
export interface IndexProgress {
  /** Files finished this run, and how many are in it (the queue grows as
   *  parses land, so `total` moves). */
  done: number;
  total: number;
  /** The file being embedded now; empty on the final callback. */
  filename: string;
  /** Pages inside that file, from `embed-status`. Zero until the document's
   *  first batch comes back: a zero denominator would draw as a finished bar,
   *  so the UI shows a count until there is a fraction to show. */
  pagesDone: number;
  totalPages: number;
}

export interface IndexResult {
  files: number;
  pages: number;
  errors: string[];
  stopped: boolean;
}

/** One file waiting its turn. */
export interface IndexJob {
  fileId: number;
  subjectId: number;
  relativePath: string;
  filename: string;
}

export interface IndexState {
  running: boolean;
  progress: IndexProgress | null;
  /** Set while a stop has been asked for but the current file has not ended. */
  stopping: boolean;
  /** The last finished run, kept so the page can report it after the fact. */
  result: IndexResult | null;
  error: string | null;
  /** A Voyage key is stored: the gate in front of auto-embed. */
  ready: boolean;

  /** Tell the store whether there is an embedder to queue work for. */
  setReady: (ready: boolean) => void;
  /** Append files and start the worker if it is idle. Already-queued paths
   *  and the file in flight are ignored, so a double event costs nothing. */
  enqueue: (jobs: IndexJob[]) => void;
  /** Queue one file by its DB row — what the pipeline table's retry uses. */
  enqueueFile: (file: DbFile) => void;
  /** Queue the whole outstanding backlog: the Index button. */
  start: () => Promise<void>;
  stop: () => void;
}

/**
 * The queue itself lives outside the store's state, and the worker reads it by
 * reference.
 *
 * Zustand state is replaced, not mutated, so a worker that had captured
 * `queue` from a snapshot would drain a copy — and a file appended by a parse
 * that finished during the run would sit in an array nobody is looking at. The
 * counters below are in the store because the UI renders them; the list is
 * private because only the worker reads it.
 */
let queue: IndexJob[] = [];
let inFlight: string | null = null;

function queuedPaths(): Set<string> {
  const paths = new Set(queue.map((job) => job.relativePath));
  if (inFlight) paths.add(inFlight);
  return paths;
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
    // The table's third stage exists only while there is an embedder behind
    // it; one owner for that fact, and this is it.
    usePipelineStore.getState().setEmbedStage(ready);
  },

  enqueue: (jobs) => {
    const seen = queuedPaths();
    const fresh = jobs.filter((job) => !seen.has(job.relativePath));
    if (fresh.length === 0) return;
    queue.push(...fresh);

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
    // The button is a bulk enqueue, not a second runner: if a parse has
    // already put a file in the queue, the backlog joins it behind.
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

  // Only a flag, plus emptying the line: the worker owns when it acts on it,
  // so the run always ends on a file boundary with its record and page rows
  // written.
  stop: () => {
    if (!get().running) return;
    queue = [];
    set({ stopping: true });
  },
}));

function blankProgress(): IndexProgress {
  return { done: 0, total: 0, filename: "", pagesDone: 0, totalPages: 0 };
}

/**
 * Page progress for the file in flight, fed from the `embed-status` event.
 *
 * Ignored for any other path, because the event is emitted for every
 * `embed_file` call — including a one-off retry made while a run is going —
 * and the settings page's bar is about the run.
 */
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
 * The worker. One at a time, until the queue is empty or something says stop.
 *
 * A failure does not end the run: the next file is very probably fine, which
 * is what `EmbedError`'s `Document` scope means. What *does* end it is a
 * failure that turns out to be **account-wide** — a spent Voyage allowance, or
 * the spend limit in Settings → Library. Those condemn every file still in the
 * queue for the identical reason, and a loop that carried on would report one
 * fact as a hundred errors, bury the five the UI shows under copies of itself
 * and — on a metered account — keep asking. `embedBlocked` is asked *after* a
 * failure rather than before each file: it is cheap (it reads
 * `voyage-usage.json` and sends nothing) but it is a reaction, not a
 * precondition. The run is allowed to try and be refused; what it is not
 * allowed to do is try 166 times for the same reason.
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
        const summary = await embedFile(job.fileId, job.subjectId, job.relativePath);
        pages += summary.pages_embedded;
      } catch (e) {
        errors.push(`${job.filename}: ${e}`);
        blocked = (await embedBlocked().catch(() => null)) != null;
      } finally {
        inFlight = null;
      }
      done += 1;
      if (blocked) {
        // Everything behind it would fail identically. The error above named
        // the reason once, which is the right number of times.
        queue = [];
        stopped = true;
        break;
      }
    }
  } catch (cause) {
    console.error("index run failed", cause);
    useIndexStore.setState({ error: String(cause) });
  } finally {
    inFlight = null;
    const total = useIndexStore.getState().progress?.total ?? done;
    useIndexStore.setState({
      running: false,
      stopping: false,
      progress: null,
      result: { files: done - errors.length, pages, errors, stopped: stopped || done < total },
    });
  }
}
