import { create } from "zustand";

/**
 * Per-file view of the full ingest pipeline:
 *
 *   download → fast parse → quality parse → embed
 *
 * Fed by `useBackendEvents` from four sources (scrape-file-start, scrape-file,
 * parse-status, and the embed statuses the sidecar posts back), and seeded
 * from the database on the Sync page so files still waiting for a stage show
 * up as backlog. A file counts as *completed* only when the quality parse AND
 * the embed that follows it are both done — never earlier, even though a
 * fast-parse embed makes it searchable before that.
 */

export type StageState = "pending" | "queued" | "active" | "done" | "error";

export interface PipelineItem {
  relativePath: string;
  subjectId: number;
  /** Course code, from `courses/<code>/…`. */
  code: string;
  filename: string;
  download: StageState;
  fast: StageState;
  quality: StageState;
  embed: StageState;
  /** Quality-parse page progress. */
  pagesDone: number;
  totalPages: number;
  /** While quality is "queued": place in the sidecar's one-at-a-time queue. */
  qualityQueuePos?: number;
  /** Embed page progress. */
  embedPagesDone: number;
  embedTotalPages: number;
  /** Stage completion times, epoch ms. Live events stamp them as they land;
   *  seeded rows carry the DB's scraped_at / parsed_at / embedded_at (the DB
   *  keeps one parse time, so a seeded quality-parsed file has no fast time). */
  downloadedAt?: number;
  fastParsedAt?: number;
  parsedAt?: number;
  embeddedAt?: number;
  /** Seeded with work outstanding and untouched by any live event yet: a run
   *  from an earlier session that never finished. Resumable — any event for
   *  the file (including a resume kicking off) clears it. */
  paused: boolean;
  error?: string;
  startedAt: number;
  updatedAt: number;
}

export type StagePatch = Partial<
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
    fast: "pending",
    quality: "pending",
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

export interface SeedRow {
  relativePath: string;
  subjectId: number;
  parseStatus: string | null;
  embedStatus: string | null;
  downloadedAt?: number;
  parsedAt?: number;
  embeddedAt?: number;
}

interface PipelineState {
  items: Record<string, PipelineItem>;

  /** Merge a stage update, creating the row if this is the first sighting. */
  touch: (relativePath: string, subjectId: number, patch: StagePatch) => void;
  /** Backfill rows from the DB without disturbing anything already live. */
  seed: (rows: SeedRow[]) => void;
  /** Mark rows stuck mid-download as failed (a run ended without their bytes). */
  failStalledDownloads: () => void;
  clearFinished: () => void;
}

export const usePipelineStore = create<PipelineState>((set) => ({
  items: {},

  touch: (relativePath, subjectId, patch) =>
    set((s) => {
      const prev = s.items[relativePath] ?? newItem(relativePath, subjectId);
      const next: PipelineItem = {
        ...prev,
        ...patch,
        // Embed events arrive without a subject id; keep the one we know.
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
        it.embeddedAt = r.embeddedAt;
        const p = r.parseStatus ?? "";
        if (p === "fast") {
          it.fast = "done";
          it.fastParsedAt = r.parsedAt;
          it.parsedAt = undefined;
        } else if (p === "queued" || p === "running") {
          // Stale from a previous session — the quality pass is outstanding.
          it.fast = "done";
          it.fastParsedAt = r.parsedAt;
          it.parsedAt = undefined;
          it.quality = "queued";
        } else if (p === "quality") {
          it.fast = "done";
          it.quality = "done";
        } else if (p.startsWith("error")) {
          it.fast = "done";
          it.quality = "error";
          it.error = p;
        }
        if (r.embedStatus === "done") it.embed = "done";
        // Outstanding work from a previous session sits paused until resumed
        // (or until a new sync touches the file).
        it.paused = !isComplete(it) && !hasFailed(it);
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
        Object.entries(s.items).filter(([, it]) => !isComplete(it) && !hasFailed(it)),
      ),
    })),
}));

// ── Derived views ─────────────────────────────────────────────────────────────

export function isComplete(it: PipelineItem): boolean {
  return it.quality === "done" && it.embed === "done";
}

export function hasFailed(it: PipelineItem): boolean {
  return (
    it.download === "error" || it.fast === "error" || it.quality === "error" || it.embed === "error"
  );
}

export type PipelinePhase = "active" | "waiting" | "paused" | "failed" | "done";

export interface StatusView {
  phase: PipelinePhase;
  /** Short word for the status pill, e.g. "Quality parse". */
  short: string;
  /** Full description for the progress column, e.g. "Quality parsing — 12/37 pages". */
  label: string;
  /** 0–100 for the current stage, or null when the stage has no page counts. */
  percent: number | null;
}

/** What the row's single progress bar should show right now. The bar tracks
 *  one stage at a time and resets as the file moves to the next stage. */
export function statusOf(it: PipelineItem): StatusView {
  if (hasFailed(it)) {
    return { phase: "failed", short: "Failed", label: it.error || "Failed", percent: null };
  }
  if (it.download === "active") {
    return { phase: "active", short: "Downloading", label: "Downloading", percent: null };
  }
  if (it.fast === "active") {
    return { phase: "active", short: "Fast parse", label: "Fast parsing", percent: null };
  }
  if (it.quality === "active") {
    const pct = it.totalPages > 0 ? (it.pagesDone / it.totalPages) * 100 : null;
    const label =
      it.totalPages > 0
        ? `Quality parsing — ${it.pagesDone}/${it.totalPages} pages`
        : "Quality parsing";
    return { phase: "active", short: "Quality parse", label, percent: pct };
  }
  if (it.embed === "active") {
    const pct = it.embedTotalPages > 0 ? (it.embedPagesDone / it.embedTotalPages) * 100 : null;
    const label =
      it.embedTotalPages > 0
        ? `Embedding — ${it.embedPagesDone}/${it.embedTotalPages} pages`
        : "Embedding";
    return { phase: "active", short: "Embedding", label, percent: pct };
  }
  if (isComplete(it)) {
    return { phase: "done", short: "Done", label: "Completed", percent: 100 };
  }
  // Leftovers from an earlier session: nothing is queued anywhere for these
  // until the user resumes them (or a new sync touches the file).
  if (it.paused) {
    const remaining =
      it.quality === "done"
        ? "embed pending"
        : it.fast === "done"
          ? "quality parse pending"
          : "parse pending";
    return { phase: "paused", short: "Paused", label: `Paused — ${remaining}`, percent: null };
  }
  if (it.quality === "queued") {
    const label = it.qualityQueuePos
      ? it.qualityQueuePos === 1
        ? "Queued for quality parse — next up"
        : `Queued for quality parse — #${it.qualityQueuePos} in line`
      : "Queued for quality parse";
    return { phase: "waiting", short: "Queued", label, percent: null };
  }
  // Below here nothing is actually queued anywhere — these are backlog rows
  // that need the next sync (or an index run) to pick them up.
  if (it.quality === "done") {
    return { phase: "waiting", short: "Waiting", label: "Waiting to embed", percent: null };
  }
  if (it.fast === "done") {
    return { phase: "waiting", short: "Waiting", label: "Waiting for quality parse", percent: null };
  }
  if (it.download === "done") {
    return { phase: "waiting", short: "Waiting", label: "Waiting to parse", percent: null };
  }
  return { phase: "waiting", short: "Waiting", label: "Waiting to download", percent: null };
}
