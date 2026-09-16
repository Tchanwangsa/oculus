import { create } from "zustand";

/**
 * Per-file view of the ingest pipeline:
 *
 *   download → parse
 *
 * Two stages, not four. The fast local tier and the embed pass that followed
 * it are gone: MinerU cloud is the only parser, so there is one parse and a
 * file is *completed* the moment it lands.
 *
 * Fed by `useBackendEvents` from three events (scrape-file-start, scrape-file,
 * parse-status) and seeded from the database on the Sync page, so files still
 * waiting for a stage show up as backlog.
 *
 * **The wire and DB string for a finished parse is still `"quality"`**, which
 * is why the seeding switch below reads it. That name outlived the tier it was
 * named after — `files.parse_status = 'quality'` is what every already-parsed
 * row in the user's library says, and renaming it would invalidate all of
 * them. The *field* here is `parse`, because inside the store there is only
 * one parse stage to name; the string stays `"quality"` forever.
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
  /** Parse page progress. */
  pagesDone: number;
  totalPages: number;
  /** While parse is "queued": place in the parse queue, when it is known. */
  parseQueuePos?: number;
  /** Stage completion times, epoch ms. Live events stamp them as they land;
   *  seeded rows carry the DB's scraped_at / parsed_at. */
  downloadedAt?: number;
  parsedAt?: number;
  /** Seeded with work outstanding and untouched by any live event yet: a run
   *  from an earlier session that never finished. Resumable — any event for
   *  the file (including a resume kicking off) clears it. */
  paused: boolean;
  /** Human-readable failure text, safe to display. */
  error?: string;
  /** Machine-readable discriminant for the failure, from the `parse-status`
   *  event's `kind`. Carried so the failure UI can say *what* went wrong
   *  rather than only that something did. */
  errorKind?: string;
  /** Could retrying **this file** ever work? `false` means it cannot —
   *  a corrupt PDF, one past the size limit. */
  errorRetryable?: boolean;
  /** Does the failure condemn every other file too (no token, a rejected
   *  token, exhausted quota)? See `useQualitySweep`, which stands down while
   *  one of these is in force rather than marching the library into it. */
  errorLatching?: boolean;
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
    parse: "pending",
    pagesDone: 0,
    totalPages: 0,
    paused: false,
    startedAt: Date.now(),
    updatedAt: Date.now(),
  };
}

export interface SeedRow {
  relativePath: string;
  subjectId: number;
  /** `files.parse_status` as stored — `"quality"` when parsed, an `error…`
   *  string when the last attempt failed, and NULL / an in-flight word from
   *  an interrupted session otherwise. */
  parseStatus: string | null;
  downloadedAt?: number;
  parsedAt?: number;
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
        // Two terminal statuses, and nothing else is worth a branch: an
        // interrupted session's `queued`/`running` is simply outstanding work,
        // which the `paused` line below already says.
        if (p === "quality") {
          it.parse = "done";
        } else if (p.startsWith("error")) {
          it.parse = "error";
          it.error = p;
        }
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
  return it.parse === "done";
}

export function hasFailed(it: PipelineItem): boolean {
  return it.download === "error" || it.parse === "error";
}

export type PipelinePhase = "active" | "waiting" | "paused" | "failed" | "done";

export interface StatusView {
  phase: PipelinePhase;
  /** Short word for the status pill, e.g. "Parsing". */
  short: string;
  /** Full description for the progress column, e.g. "Parsing — 12/37 pages". */
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
  if (it.parse === "active") {
    const pct = it.totalPages > 0 ? (it.pagesDone / it.totalPages) * 100 : null;
    const label = it.totalPages > 0 ? `Parsing — ${it.pagesDone}/${it.totalPages} pages` : "Parsing";
    return { phase: "active", short: "Parsing", label, percent: pct };
  }
  if (isComplete(it)) {
    return { phase: "done", short: "Done", label: "Completed", percent: 100 };
  }
  // Leftovers from an earlier session: nothing is queued anywhere for these
  // until the user resumes them (or a new sync touches the file).
  if (it.paused) {
    return { phase: "paused", short: "Paused", label: "Paused — parse pending", percent: null };
  }
  // Parses are submitted in batches, so there is still a real line to be in.
  if (it.parse === "queued") {
    const label = it.parseQueuePos
      ? it.parseQueuePos === 1
        ? "Queued to parse — next up"
        : `Queued to parse — #${it.parseQueuePos} in line`
      : "Queued to parse";
    return { phase: "waiting", short: "Queued", label, percent: null };
  }
  // Below here nothing is actually queued anywhere — these are backlog rows
  // that need the next sync (or the sweep) to pick them up.
  if (it.download === "done") {
    return { phase: "waiting", short: "Waiting", label: "Waiting to parse", percent: null };
  }
  return { phase: "waiting", short: "Waiting", label: "Waiting to download", percent: null };
}
