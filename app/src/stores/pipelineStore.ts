import { create } from "zustand";

/**
 * Per-file view of the ingest pipeline: download → parse → embed. The embed
 * stage applies only when a Voyage key is stored (`embedStage`); without one a
 * parsed file is finished.
 *
 * Fed by `useBackendEvents` (scrape-file-start, scrape-file, parse-status,
 * embed-status) and seeded from the DB on the Sync page. A finished parse is
 * `"quality"` on the wire and in `files.parse_status`; a finished embed is
 * `"done"`.
 */

export type StageState = "pending" | "queued" | "active" | "done" | "error";

export interface PipelineItem {
  relativePath: string;
  subjectId: number;
  /** Course code, from `courses/<code>/…`. */
  code: string;
  filename: string;
  download: StageState;
  parse: StageState;
  embed: StageState;
  /** Parse page progress. */
  pagesDone: number;
  totalPages: number;
  /** Separate from parse's pair, or the embed stage would start at 100%. */
  embedPagesDone: number;
  embedTotalPages: number;
  /** While parse is "queued": place in the parse queue, when it is known. */
  parseQueuePos?: number;
  /** Stage completion times, epoch ms (seeded from the DB's `*_at`). */
  downloadedAt?: number;
  parsedAt?: number;
  embeddedAt?: number;
  /** Seeded with work outstanding from an earlier session; any live event
   *  for the file clears it. */
  paused: boolean;
  /** Display text for whichever stage failed (only one can). */
  error?: string;
  /** The failing event's `kind`; parse and embed share the vocabulary. */
  errorKind?: string;
  /** `false`: retrying this file can never work. */
  errorRetryable?: boolean;
  /** The cause blocks every file (see `useQualitySweep`). */
  errorLatching?: boolean;
  startedAt: number;
  updatedAt: number;
}

type StagePatch = Partial<
  Omit<PipelineItem, "relativePath" | "subjectId" | "code" | "filename" | "startedAt" | "updatedAt">
>;

function newItem(relativePath: string, subjectId: number): PipelineItem {
  const parts = relativePath.split("/");
  return {
    relativePath,
    subjectId,
    code: parts[0] === "courses" ? (parts[1] ?? "") : "",
    filename: parts[parts.length - 1] ?? relativePath,
    download: "pending",
    parse: "pending",
    embed: "pending",
    pagesDone: 0,
    totalPages: 0,
    embedPagesDone: 0,
    embedTotalPages: 0,
    paused: false,
    startedAt: Date.now(),
    updatedAt: Date.now(),
  };
}

interface SeedRow {
  relativePath: string;
  subjectId: number;
  /** `files.parse_status`: `"quality"`, `error…`, or NULL / an in-flight word. */
  parseStatus: string | null;
  /** Read only for its failure: the column does not know which embedding
   *  space it was set in, so completion comes from the page counts. */
  embedStatus: string | null;
  /** Page rows for the file, and how many carry a current-space vector. */
  pagesTotal?: number;
  pagesCurrent?: number;
  downloadedAt?: number;
  parsedAt?: number;
  embeddedAt?: number;
}

interface PipelineState {
  items: Record<string, PipelineItem>;
  /** False until a Voyage key is stored, so no phantom embed backlog is drawn.
   *  Set from `indexStore`. */
  embedStage: boolean;
  setEmbedStage: (on: boolean) => void;

  /** Merge a stage update, creating the row if this is the first sighting. */
  touch: (relativePath: string, subjectId: number, patch: StagePatch) => void;
  /** Backfill rows from the DB without disturbing anything already live. */
  seed: (rows: SeedRow[]) => void;
  /** Mark rows stuck mid-download as failed (a run ended without their bytes). */
  failStalledDownloads: () => void;
  clearFinished: () => void;
}

export const usePipelineStore = create<PipelineState>((set, get) => ({
  items: {},
  embedStage: false,

  setEmbedStage: (on) => {
    if (get().embedStage !== on) set({ embedStage: on });
  },

  touch: (relativePath, subjectId, patch) =>
    set((s) => {
      const prev = s.items[relativePath] ?? newItem(relativePath, subjectId);
      const next: PipelineItem = {
        ...prev,
        ...patch,
        // Keep a subject id we already know over a missing one.
        subjectId: prev.subjectId || subjectId,
        // Any live event means the file is moving again.
        paused: false,
        updatedAt: Date.now(),
      };
      return { items: { ...s.items, [relativePath]: next } };
    }),

  seed: (rows) =>
    set((s) => {
      const items = { ...s.items };
      for (const r of rows) {
        if (items[r.relativePath]) continue;
        const it = newItem(r.relativePath, r.subjectId);
        it.download = "done"; // it's in the DB, so it's on disk
        it.downloadedAt = r.downloadedAt;
        it.parsedAt = r.parsedAt;
        const p = r.parseStatus ?? "";
        // An interrupted `queued`/`running` is just outstanding (`paused`).
        if (p === "quality") {
          it.parse = "done";
        } else if (p.startsWith("error")) {
          it.parse = "error";
          it.error = p;
        }
        // Coverage, not the status column: a retired model's vectors share
        // the table and would otherwise read as done.
        const total = r.pagesTotal ?? 0;
        const current = r.pagesCurrent ?? 0;
        if (total > 0 && current >= total) {
          it.embed = "done";
          it.embedPagesDone = current;
          it.embedTotalPages = total;
          it.embeddedAt = r.embeddedAt;
        } else if ((r.embedStatus ?? "").startsWith("error")) {
          if (it.parse === "done") {
            it.embed = "error";
            it.error = it.error ?? "Embedding failed";
          }
        }
        // Judged with the embed stage on regardless of `embedStage`, so a key
        // saved after seeding needs no re-seed; harmless when off, since
        // `statusOf` checks completeness before `paused`.
        it.paused = !isComplete(it, true) && !hasFailed(it);
        items[r.relativePath] = it;
      }
      return { items };
    }),

  failStalledDownloads: () =>
    set((s) => {
      const items = { ...s.items };
      for (const [k, it] of Object.entries(items)) {
        if (it.download === "active") {
          items[k] = {
            ...it,
            download: "error",
            error: "Download did not complete",
            updatedAt: Date.now(),
          };
        }
      }
      return { items };
    }),

  clearFinished: () =>
    set((s) => ({
      items: Object.fromEntries(
        Object.entries(s.items).filter(
          ([, it]) => !isComplete(it, s.embedStage) && !hasFailed(it),
        ),
      ),
    })),
}));

// ── Derived views ─────────────────────────────────────────────────────────────

/** `embedStage` defaults to false: with no embedder, parsed is finished. */
export function isComplete(it: PipelineItem, embedStage = false): boolean {
  return embedStage ? it.embed === "done" : it.parse === "done";
}

export function hasFailed(it: PipelineItem): boolean {
  return it.download === "error" || it.parse === "error" || it.embed === "error";
}

export type PipelinePhase = "active" | "waiting" | "paused" | "failed" | "done";

export interface StatusView {
  phase: PipelinePhase;
  /** Status pill word, e.g. "Parsing". */
  short: string;
  /** Progress column text, e.g. "Parsing — 12/37 pages". */
  label: string;
  /** 0–100 for the current stage; null without page counts. */
  percent: number | null;
}

/** The row's single progress bar, which tracks one stage at a time. */
export function statusOf(it: PipelineItem, embedStage = false): StatusView {
  if (hasFailed(it)) {
    return { phase: "failed", short: "Failed", label: it.error || "Failed", percent: null };
  }
  if (it.download === "active") {
    return { phase: "active", short: "Downloading", label: "Downloading", percent: null };
  }
  if (it.parse === "active") {
    const pct = it.totalPages > 0 ? (it.pagesDone / it.totalPages) * 100 : null;
    const label = it.totalPages > 0 ? `Parsing — ${it.pagesDone}/${it.totalPages} pages` : "Parsing";
    return { phase: "active", short: "Parsing", label, percent: pct };
  }
  // An embed can run for an hour; its page counter is the proof it is alive.
  if (it.embed === "active") {
    const pct =
      it.embedTotalPages > 0 ? (it.embedPagesDone / it.embedTotalPages) * 100 : null;
    const label =
      it.embedTotalPages > 0
        ? `Embedding — ${it.embedPagesDone}/${it.embedTotalPages} pages`
        : "Embedding";
    return { phase: "active", short: "Embedding", label, percent: pct };
  }
  if (isComplete(it, embedStage)) {
    return { phase: "done", short: "Done", label: "Completed", percent: 100 };
  }
  if (it.embed === "queued") {
    return { phase: "waiting", short: "Queued", label: "Queued to embed", percent: null };
  }
  if (it.paused) {
    const stage = it.parse === "done" ? "embed" : "parse";
    return { phase: "paused", short: "Paused", label: `Paused — ${stage} pending`, percent: null };
  }
  if (it.parse === "queued") {
    const label = it.parseQueuePos
      ? it.parseQueuePos === 1
        ? "Queued to parse — next up"
        : `Queued to parse — #${it.parseQueuePos} in line`
      : "Queued to parse";
    return { phase: "waiting", short: "Queued", label, percent: null };
  }
  // Backlog: nothing is queued; the next sync or the sweep picks these up.
  if (it.parse === "done") {
    return { phase: "waiting", short: "Waiting", label: "Waiting to embed", percent: null };
  }
  if (it.download === "done") {
    return { phase: "waiting", short: "Waiting", label: "Waiting to parse", percent: null };
  }
  return { phase: "waiting", short: "Waiting", label: "Waiting to download", percent: null };
}
