import { create } from "zustand";

/**
 * Per-file view of the ingest pipeline:
 *
 *   download → parse → embed
 *
 * Three stages. The fourth the table used to draw — a fast local parse tier —
 * is gone for good; the third is back, and is not the one that was removed.
 * That one was the Python sidecar's local embedder running inside the parse
 * queue. This one is a metered cloud call that a file reaches only after its
 * parse has landed, and it is **conditional**: with no Voyage key stored there
 * is no embedder to wait for, so the stage does not apply and a parsed file is
 * finished at two dots. `embedStage` is that switch, and every derived view
 * below takes it rather than assuming.
 *
 * Fed by `useBackendEvents` from four events (scrape-file-start, scrape-file,
 * parse-status, embed-status) and seeded from the database on the Sync page,
 * so files still waiting for a stage show up as backlog.
 *
 * **The wire and DB string for a finished parse is still `"quality"`**, which
 * is why the seeding switch below reads it. That name outlived the tier it was
 * named after — `files.parse_status = 'quality'` is what every already-parsed
 * row in the user's library says, and renaming it would invalidate all of
 * them. The embed stage inherited none of that: its terminal success is
 * `"done"`, because nothing was ever written down under another name.
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
  /** Embed page progress — a separate pair, because the two stages count the
   *  same document twice and a shared counter would make the second stage
   *  start at 100%. */
  embedPagesDone: number;
  embedTotalPages: number;
  /** While parse is "queued": place in the parse queue, when it is known. */
  parseQueuePos?: number;
  /** Stage completion times, epoch ms. Live events stamp them as they land;
   *  seeded rows carry the DB's scraped_at / parsed_at / embedded_at. */
  downloadedAt?: number;
  parsedAt?: number;
  embeddedAt?: number;
  /** Seeded with work outstanding and untouched by any live event yet: a run
   *  from an earlier session that never finished. Resumable — any event for
   *  the file (including a resume kicking off) clears it. */
  paused: boolean;
  /** Human-readable failure text, safe to display. Whichever stage failed —
   *  they cannot both be failing, since the second never starts until the
   *  first is done. */
  error?: string;
  /** Machine-readable discriminant for the failure, from the `parse-status`
   *  or `embed-status` event's `kind`. Carried so the failure UI can say
   *  *what* went wrong rather than only that something did. The two seams
   *  speak the same vocabulary on purpose. */
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
  /** `files.parse_status` as stored — `"quality"` when parsed, an `error…`
   *  string when the last attempt failed, and NULL / an in-flight word from
   *  an interrupted session otherwise. */
  parseStatus: string | null;
  /** `files.embed_status` as stored. Read **only** for its failure: a file is
   *  embedded when its pages are covered in the current space, which is what
   *  the two counts below say, and this column has no memory of which space
   *  it was set in. */
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
  /**
   * Does the embed stage apply at all? False until a Voyage key is stored —
   * the app will not start an embed without one, so drawing 161 rows as
   * "waiting to embed" would be inventing a backlog for a stage that is
   * switched off. Set from `indexStore`, which owns readiness.
   */
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
        // Two terminal statuses, and nothing else is worth a branch: an
        // interrupted session's `queued`/`running` is simply outstanding work,
        // which the `paused` line below already says.
        if (p === "quality") {
          it.parse = "done";
        } else if (p.startsWith("error")) {
          it.parse = "error";
          it.error = p;
        }
        // The embed stage is read off coverage, never off the status column:
        // vectors from a retired model sit in the same table and would
        // otherwise read as done. A file with no page rows has not been parsed,
        // so there is nothing to be covered *of* and it stays pending.
        const total = r.pagesTotal ?? 0;
        const current = r.pagesCurrent ?? 0;
        if (total > 0 && current >= total) {
          it.embed = "done";
          it.embedPagesDone = current;
          it.embedTotalPages = total;
          it.embeddedAt = r.embeddedAt;
        } else if ((r.embedStatus ?? "").startsWith("error")) {
          // Only meaningful once the parse is done; a file that never parsed
          // cannot have failed to embed in a way worth showing.
          if (it.parse === "done") {
            it.embed = "error";
            it.error = it.error ?? "Embedding failed";
          }
        }
        // Outstanding work from a previous session sits paused until resumed
        // (or until a new sync touches the file).
        //
        // Asked of all three stages whatever `embedStage` currently says, so
        // that a key saved *after* the table was seeded does not leave a
        // library of rows that were decided under the old answer. It costs
        // nothing when the stage is off: `statusOf` checks completeness before
        // it looks at `paused`, so a parsed file with no embedder still reads
        // as done.
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

/**
 * `embedStage` defaults to false in every derived view, and that is the safe
 * direction: with no embedder configured a parsed file is finished, which is
 * what the table said before this stage existed.
 */
export function isComplete(it: PipelineItem, embedStage = false): boolean {
  return embedStage ? it.embed === "done" : it.parse === "done";
}

export function hasFailed(it: PipelineItem): boolean {
  return it.download === "error" || it.parse === "error" || it.embed === "error";
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
  // An embed in flight outranks everything below: it is the only stage that
  // routinely runs for an hour on one file, and on the free Voyage programme
  // (~2.8 pages a minute) the page counter is the only proof it is alive.
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
  // Leftovers from an earlier session: nothing is queued anywhere for these
  // until the user resumes them (or a new sync touches the file).
  if (it.paused) {
    const stage = it.parse === "done" ? "embed" : "parse";
    return { phase: "paused", short: "Paused", label: `Paused — ${stage} pending`, percent: null };
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
  if (it.parse === "done") {
    return { phase: "waiting", short: "Waiting", label: "Waiting to embed", percent: null };
  }
  if (it.download === "done") {
    return { phase: "waiting", short: "Waiting", label: "Waiting to parse", percent: null };
  }
  return { phase: "waiting", short: "Waiting", label: "Waiting to download", percent: null };
}
