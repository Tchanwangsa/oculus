import { hasFailed } from "./status";

/** `skipped` is the parse stage's alone. */
export type StageState = "pending" | "queued" | "active" | "done" | "error" | "skipped";

/** A running cloud parse's sub-step (`parse-status`'s `phase`): its batch is
 *  submitted and another file uploads first, its bytes are going up, or
 *  MinerU is extracting. The local engine only ever reports `processing`. */
export type ParsePhase = "upload_wait" | "uploading" | "processing";

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

export function newItem(relativePath: string, subjectId: number): PipelineItem {
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

/** A row touched this recently may hold state the DB has not caught up with
 *  (e.g. a download whose upsert is still in flight), so `seed` leaves it be. */
export const FRESH_MS = 60_000;

const NO_ERROR = {
  error: undefined,
  errorKind: undefined,
  errorRetryable: undefined,
  errorLatching: undefined,
} as const;

/** A stage is mid-run, so live events own the row. */
export function inFlight(it: PipelineItem): boolean {
  return [it.download, it.parse, it.embed].some((st) => st === "active" || st === "queued");
}

/** The error fields are one per row: drop them once no stage has failed. */
export function settleError(it: PipelineItem): PipelineItem {
  return hasFailed(it) ? it : { ...it, ...NO_ERROR };
}
