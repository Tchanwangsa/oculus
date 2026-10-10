import { newItem, inFlight, settleError, FRESH_MS, type PipelineItem } from "./item";
import { hasFailed, isComplete } from "./status";

export interface SeedRow {
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

/** A row as the DB alone describes it. */
export function seededItem(r: SeedRow): PipelineItem {
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
export function mergeSeed(live: PipelineItem, db: PipelineItem, now: number): PipelineItem {
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
