import { create } from "zustand";

/**
 * Per-file view of the ingest pipeline: download → parse → embed. The embed
 * stage applies only when a Voyage key is stored (`embedStage`); without one a
 * parsed file is finished.
 *
 * Fed by `useBackendEvents` (scrape-file-start, scrape-file, parse-status,
 * embed-status) and seeded from the DB on the Sync page. A finished parse is
 * `"quality"` on the wire and in `files.parse_status`; a finished embed is
 * `"done"`. A parse the user skipped is `"skipped"` in both, and the row's
 * parse stage; it is settled, never a failure and never paused.
 */

/** `skipped` is the parse stage's alone. */
export type StageState = "pending" | "queued" | "active" | "done" | "error" | "skipped";

/** A running cloud parse's sub-step (`parse-status`'s `phase`): its batch is
 *  submitted and another file uploads first, its bytes are going up, or
 *  MinerU is extracting. The local engine only ever reports `processing`. */
export type ParsePhase = "upload_wait" | "uploading" | "processing";

/** Below this much sampling, an upload's rate is noise. */
const ETA_MIN_SAMPLE_MS = 10_000;

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
  /** While an active embed is held by a rate limit: when it expects to
   *  resume (epoch ms) and why, e.g. "rate-limited by Voyage". Any embed
   *  event without them clears them. */
  embedWaitingUntil?: number;
  embedWaitingReason?: string;
  /** While parse is "queued": place in the parse queue, when it is known. */
  parseQueuePos?: number;
  /** While parse is "active". Absent: processing. */
  parsePhase?: ParsePhase;
  /** A cloud upload's bytes; the total is known from `upload_wait` on and
   *  kept once the upload is done, so the row can say what went up. */
  bytesDone?: number;
  bytesTotal?: number;
  /** The first and the latest `uploading` sample (epoch ms, and bytes at
   *  the first), for the rate behind `uploadEta`. */
  uploadFirstAt?: number;
  uploadFirstBytes?: number;
  uploadSampleAt?: number;
  /** Stage completion times, epoch ms (seeded from the DB's `*_at`).
   *  `uploadedAt` and `skippedAt` are live-only. */
  downloadedAt?: number;
  uploadedAt?: number;
  parsedAt?: number;
  embeddedAt?: number;
  skippedAt?: number;
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

/** Clears a parse's upload sub-state: a new parse, or new bytes. */
export const NO_UPLOAD = {
  parsePhase: undefined,
  bytesDone: undefined,
  bytesTotal: undefined,
  uploadFirstAt: undefined,
  uploadFirstBytes: undefined,
  uploadSampleAt: undefined,
  uploadedAt: undefined,
} as const;

/** The patch for a `running` parse event: its phase and bytes, the upload's
 *  rate samples (restarted whenever an upload starts or its bytes go
 *  backwards), and when the upload finished. */
export function runningPatch(
  prev: PipelineItem | undefined,
  ev: { phase?: ParsePhase; bytes_done?: number; bytes_total?: number },
  now: number,
): StagePatch {
  const phase = ev.phase ?? "processing";
  const bytesTotal = ev.bytes_total ?? prev?.bytesTotal;
  const patch: StagePatch = { parsePhase: phase, bytesTotal };
  if (phase === "uploading") {
    const done = ev.bytes_done ?? 0;
    const continuing =
      prev?.parsePhase === "uploading" &&
      prev.uploadFirstAt != null &&
      done >= (prev.uploadFirstBytes ?? 0);
    patch.bytesDone = done;
    patch.uploadFirstAt = continuing ? prev?.uploadFirstAt : now;
    patch.uploadFirstBytes = continuing ? prev?.uploadFirstBytes : done;
    patch.uploadSampleAt = now;
  } else if (phase === "processing") {
    const uploaded = prev?.parsePhase === "uploading" || prev?.parsePhase === "upload_wait";
    if (uploaded) {
      patch.uploadedAt = now;
      if (bytesTotal != null) patch.bytesDone = bytesTotal;
    }
  } else {
    patch.bytesDone = ev.bytes_done ?? 0;
  }
  return patch;
}

/** Milliseconds an upload has left at its latest sample, or null until
 *  `ETA_MIN_SAMPLE_MS` of samples show it moving. */
export function uploadEta(it: PipelineItem): number | null {
  if (it.parse !== "active" || it.parsePhase !== "uploading") return null;
  const { uploadFirstAt: t0, uploadSampleAt: t1, bytesDone, bytesTotal } = it;
  if (t0 == null || t1 == null || bytesDone == null || !bytesTotal) return null;
  const span = t1 - t0;
  const moved = bytesDone - (it.uploadFirstBytes ?? 0);
  if (span < ETA_MIN_SAMPLE_MS || moved <= 0) return null;
  return Math.max(0, bytesTotal - bytesDone) / (moved / span);
}

/** "~2 h left", "~1.5 h left", "~40 min left", "~3 min left", "under a
 *  minute left": coarser as the wait grows, since the rate wobbles. */
export function fmtEta(ms: number): string {
  const mins = ms / 60_000;
  if (mins < 1) return "under a minute left";
  if (mins < 10) return `~${Math.round(mins)} min left`;
  if (mins < 55) return `~${Math.round(mins / 5) * 5} min left`;
  const hours = mins / 60;
  if (hours < 10) {
    const h = Math.max(1, Math.round(hours * 2) / 2);
    return `~${h} h left`;
  }
  return `~${Math.round(hours)} h left`;
}

/** A row touched this recently may hold state the DB has not caught up with
 *  (e.g. a download whose upsert is still in flight), so `seed` leaves it be. */
const FRESH_MS = 60_000;

const NO_ERROR = {
  error: undefined,
  errorKind: undefined,
  errorRetryable: undefined,
  errorLatching: undefined,
} as const;

/** A stage is mid-run, so live events own the row. */
function inFlight(it: PipelineItem): boolean {
  return [it.download, it.parse, it.embed].some((st) => st === "active" || st === "queued");
}

/** The error fields are one per row: drop them once no stage has failed. */
function settleError(it: PipelineItem): PipelineItem {
  return hasFailed(it) ? it : { ...it, ...NO_ERROR };
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
  /** Paths "Clear finished" removed; `seed` leaves them out while they are
   *  still complete, and a live `touch` brings them back. */
  cleared: Record<string, true>;

  /** Merge a stage update, creating the row if this is the first sighting. */
  touch: (relativePath: string, subjectId: number, patch: StagePatch) => void;
  /** A sync found the file unchanged on disk. Not a sign of movement: never
   *  creates a row, and only settles a download the row still shows as
   *  unfinished. */
  confirmDownload: (relativePath: string) => void;
  /** Merge the DB's view: new rows are added, idle rows advance to where the
   *  DB is further along, and rows for files no longer on record are dropped. */
  seed: (rows: SeedRow[]) => void;
  /** Mark rows stuck mid-download as failed (a run ended without their bytes). */
  failStalledDownloads: () => void;
  /** Drop completed rows. Failed ones stay, since they need attention. */
  clearFinished: () => void;
}

export const usePipelineStore = create<PipelineState>((set, get) => ({
  items: {},
  embedStage: false,
  cleared: {},

  setEmbedStage: (on) => {
    if (get().embedStage !== on) set({ embedStage: on });
  },

  touch: (relativePath, subjectId, patch) =>
    set((s) => {
      const prev = s.items[relativePath] ?? newItem(relativePath, subjectId);
      // A parse or embed event means the bytes are on disk, even when it is
      // the first event this session saw for the file, or a run marked its
      // download failed.
      const onDisk =
        (prev.download === "pending" || prev.download === "error") &&
        patch.download === undefined &&
        (patch.parse !== undefined || patch.embed !== undefined);
      let next: PipelineItem = {
        ...prev,
        ...(onDisk ? { download: "done" as const } : {}),
        ...patch,
        // Keep a subject id we already know over a missing one.
        subjectId: prev.subjectId || subjectId,
        // Any live event means the file is moving again.
        paused: false,
        updatedAt: Date.now(),
      };
      if (prev.download === "error" && next.download === "done") next = settleError(next);
      let cleared = s.cleared;
      if (cleared[relativePath]) {
        cleared = { ...cleared };
        delete cleared[relativePath];
      }
      return { items: { ...s.items, [relativePath]: next }, cleared };
    }),

  confirmDownload: (relativePath) =>
    set((s) => {
      const prev = s.items[relativePath];
      if (!prev || prev.download === "done") return {};
      const next = settleError({ ...prev, download: "done" });
      return { items: { ...s.items, [relativePath]: next } };
    }),

  seed: (rows) =>
    set((s) => {
      const now = Date.now();
      const items: Record<string, PipelineItem> = {};
      const cleared: Record<string, true> = {};
      for (const r of rows) {
        const db = seededItem(r);
        const live = s.items[r.relativePath];
        if (live) {
          items[r.relativePath] = mergeSeed(live, db, now);
        } else if (s.cleared[r.relativePath] && isComplete(db, s.embedStage)) {
          cleared[r.relativePath] = true;
        } else {
          items[r.relativePath] = db;
        }
      }
      // A file gone from the DB was deleted or renamed, unless its row is
      // still being written.
      for (const [k, it] of Object.entries(s.items)) {
        if (k in items) continue;
        if (inFlight(it) || now - it.updatedAt < FRESH_MS) items[k] = it;
      }
      return { items, cleared };
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
    set((s) => {
      const items: Record<string, PipelineItem> = {};
      const cleared = { ...s.cleared };
      for (const [k, it] of Object.entries(s.items)) {
        if (isComplete(it, s.embedStage) && !hasFailed(it, s.embedStage)) cleared[k] = true;
        else items[k] = it;
      }
      return { items, cleared };
    }),
}));

/** A row as the DB alone describes it. */
function seededItem(r: SeedRow): PipelineItem {
  const it = newItem(r.relativePath, r.subjectId);
  it.download = "done"; // it's in the DB, so it's on disk
  it.downloadedAt = r.downloadedAt;
  it.parsedAt = r.parsedAt;
  const p = r.parseStatus ?? "";
  // An interrupted `queued`/`running` is just outstanding (`paused`).
  if (p === "quality") {
    it.parse = "done";
  } else if (p === "skipped") {
    it.parse = "skipped";
  } else if (p.startsWith("error")) {
    it.parse = "error";
    it.error = /^error:\s*(\S[\s\S]*)$/.exec(p)?.[1] ?? "Parse failed";
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
  // Ordered by when the file last moved, not when the page opened.
  const last = Math.max(r.downloadedAt ?? 0, r.parsedAt ?? 0, r.embeddedAt ?? 0);
  it.startedAt = last;
  it.updatedAt = last;
  return it;
}

/** Advance an idle live row to the DB's state where the DB is further along;
 *  never regress a live stage. */
function mergeSeed(live: PipelineItem, db: PipelineItem, now: number): PipelineItem {
  let it = { ...live };
  if (!inFlight(live) && now - live.updatedAt >= FRESH_MS) {
    if (it.download === "pending") it.download = "done";
    if (db.parse === "done" && it.parse !== "done") {
      it.parse = "done";
    } else if (db.parse === "skipped" && it.parse !== "skipped") {
      it.parse = "skipped";
    } else if (db.parse === "error" && it.parse === "pending") {
      it.parse = "error";
    }
    if (db.embed === "done" && it.embed !== "done") {
      it.embed = "done";
      it.embedPagesDone = db.embedPagesDone;
      it.embedTotalPages = db.embedTotalPages;
      if (it.parse === "pending") it.parse = "done";
    } else if (db.embed === "error" && it.embed === "pending" && it.parse === "done") {
      it.embed = "error";
    }
    it = settleError(it);
    if (hasFailed(it)) it.error ??= db.error;
    it.paused = !isComplete(it, true) && !hasFailed(it);
  }
  // A stamp only for a stage that is done, or a re-parse would show the
  // previous parse's time.
  it.downloadedAt ??= db.downloadedAt;
  if (it.parse === "done") it.parsedAt ??= db.parsedAt;
  if (it.embed === "done") it.embeddedAt ??= db.embeddedAt;
  return it;
}

// ── Derived views ─────────────────────────────────────────────────────────────

/** Nothing more will happen to the row: finished, or its parse skipped.
 *  `embedStage` defaults to false: with no embedder, parsed is finished. */
export function isComplete(it: PipelineItem, embedStage = false): boolean {
  if (it.parse === "skipped") return true;
  return embedStage ? it.embed === "done" : it.parse === "done";
}

/** `embedStage` defaults to true; with it off, a failed embed is not drawn. */
export function hasFailed(it: PipelineItem, embedStage = true): boolean {
  return (
    it.download === "error" || it.parse === "error" || (embedStage && it.embed === "error")
  );
}

export type PipelinePhase = "active" | "waiting" | "paused" | "failed" | "skipped" | "done";

export interface StatusView {
  phase: PipelinePhase;
  /** Status pill word, e.g. "Parsing". */
  short: string;
  /** The progress caption, e.g. "Parsing — 12/37 pages". */
  label: string;
  /** 0–100 for the current stage; null without page counts. */
  percent: number | null;
  /** Set while the active embed is held by a rate limit: epoch ms it expects
   *  to resume. The phase stays "active", so the row keeps its place. */
  resumesAt?: number;
}

/** "40 s", "2:05"; never negative. Short enough for the status column. */
export function fmtWait(ms: number): string {
  const secs = Math.max(0, Math.ceil(ms / 1000));
  if (secs < 60) return `${secs} s`;
  return `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;
}

/** "resumes in 40 s", or "resuming…" once the expected moment has passed. */
export function fmtResume(until: number, now: number): string {
  return until > now ? `resumes in ${fmtWait(until - now)}` : "resuming…";
}

/** The row's single progress bar, which tracks one stage at a time. `now`
 *  only words a rate-limit countdown; it never changes the phase. */
export function statusOf(it: PipelineItem, embedStage = false, now = Date.now()): StatusView {
  if (hasFailed(it, embedStage)) {
    return { phase: "failed", short: "Failed", label: it.error || "Failed", percent: null };
  }
  if (it.download === "active") {
    return { phase: "active", short: "Downloading", label: "Downloading", percent: null };
  }
  if (it.parse === "skipped") {
    // The pill says "Skipped"; the caption says what that means.
    return { phase: "skipped", short: "Skipped", label: "Not parsed until you ask", percent: null };
  }
  if (it.parse === "active" && it.parsePhase === "upload_wait") {
    return { phase: "active", short: "Waiting", label: "Waiting for upload", percent: null };
  }
  if (it.parse === "active" && it.parsePhase === "uploading") {
    const total = it.bytesTotal ?? 0;
    const done = Math.min(it.bytesDone ?? 0, total);
    const label = total > 0 ? `Uploading — ${fmtMb(done)} of ${fmtMb(total)} MB` : "Uploading";
    return {
      phase: "active",
      short: "Uploading",
      label,
      percent: total > 0 ? (done / total) * 100 : null,
    };
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
    const pages =
      it.embedTotalPages > 0 ? `${it.embedPagesDone}/${it.embedTotalPages} pages` : "";
    if (it.embedWaitingUntil != null) {
      const reason = it.embedWaitingReason || "rate-limited";
      const label =
        `${reason[0].toUpperCase()}${reason.slice(1)} — ${fmtResume(it.embedWaitingUntil, now)}` +
        (pages ? ` · ${pages}` : "");
      return {
        phase: "active",
        short: "Rate-limited",
        label,
        percent: pct,
        resumesAt: it.embedWaitingUntil,
      };
    }
    const label = pages ? `Embedding — ${pages}` : "Embedding";
    return { phase: "active", short: "Embedding", label, percent: pct };
  }
  if (isComplete(it, embedStage)) {
    // The pill says "Done"; the caption says what was done.
    const embedded = embedStage && it.embed === "done";
    const pages = embedded ? it.embedTotalPages : it.totalPages;
    const what = embedded ? "Parsed and embedded" : "Parsed";
    const label = pages > 0 ? `${what} — ${pages} page${pages === 1 ? "" : "s"}` : what;
    return { phase: "done", short: "Done", label, percent: 100 };
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

/** Megabytes to one decimal, the unit an upload's caption counts in. */
export function fmtMb(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1);
}
