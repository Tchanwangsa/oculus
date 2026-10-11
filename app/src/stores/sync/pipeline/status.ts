import { isPdfBacked, isSheetFile } from "@/lib/files/fileTypes";
import type { PipelineItem } from "./item";

/** Whether this row has an embed stage: the app's `embedStage`, and only for
 *  a PDF-backed file — a spreadsheet's text is never embedded. */
export function embedsIn(it: Pick<PipelineItem, "filename">, embedStage: boolean): boolean {
  return embedStage && isPdfBacked(it.filename);
}

/** Nothing more will happen to the row: finished, or its parse skipped.
 *  `embedStage` defaults to false: with no embedder, parsed is finished. */
export function isComplete(it: PipelineItem, embedStage = false): boolean {
  if (it.parse === "skipped") return true;
  return embedsIn(it, embedStage) ? it.embed === "done" : it.parse === "done";
}

/** `embedStage` defaults to true; with it off, a failed embed is not drawn. */
export function hasFailed(it: PipelineItem, embedStage = true): boolean {
  return (
    it.download === "error" ||
    it.parse === "error" ||
    (embedsIn(it, embedStage) && it.embed === "error")
  );
}

export type PipelinePhase = "active" | "waiting" | "paused" | "failed" | "skipped" | "done";

export interface StatusView {
  phase: PipelinePhase;
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
  const sheet = isSheetFile(it.filename);
  if (hasFailed(it, embedStage)) {
    return { phase: "failed", label: it.error || "Failed", percent: null };
  }
  if (it.download === "active") {
    return { phase: "active", label: "Downloading", percent: null };
  }
  if (it.parse === "skipped") {
    return { phase: "skipped", label: "Not parsed until you ask", percent: null };
  }
  if (it.parse === "active" && it.parsePhase === "upload_wait") {
    return { phase: "active", label: "Waiting for upload", percent: null };
  }
  if (it.parse === "active" && it.parsePhase === "uploading") {
    const total = it.bytesTotal ?? 0;
    const done = Math.min(it.bytesDone ?? 0, total);
    const label = total > 0 ? `Uploading — ${fmtMb(done)} of ${fmtMb(total)} MB` : "Uploading";
    return {
      phase: "active",
      label,
      percent: total > 0 ? (done / total) * 100 : null,
    };
  }
  if (it.parse === "active") {
    const pct = it.totalPages > 0 ? (it.pagesDone / it.totalPages) * 100 : null;
    const label = it.totalPages > 0 ? `Parsing — ${it.pagesDone}/${it.totalPages} pages` : "Parsing";
    return { phase: "active", label, percent: pct };
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
        label,
        percent: pct,
        resumesAt: it.embedWaitingUntil,
      };
    }
    const label = pages ? `Embedding — ${pages}` : "Embedding";
    return { phase: "active", label, percent: pct };
  }
  if (isComplete(it, embedStage)) {
    // Searchable once embedded; without the embed stage, parsed is the end.
    const label = sheet
      ? "Converted to text"
      : embedStage && it.embed === "done" ? "Indexed" : "Parsed";
    return { phase: "done", label, percent: 100 };
  }
  if (it.embed === "queued") {
    return { phase: "waiting", label: "Queued to embed", percent: null };
  }
  if (it.paused) {
    const stage = it.parse === "done" ? "embed" : sheet ? "conversion" : "parse";
    return { phase: "paused", label: `Paused — ${stage} pending`, percent: null };
  }
  if (it.parse === "queued") {
    const label = it.parseQueuePos
      ? it.parseQueuePos === 1
        ? "Queued to parse — next up"
        : `Queued to parse — #${it.parseQueuePos} in line`
      : "Queued to parse";
    return { phase: "waiting", label, percent: null };
  }
  // Backlog: nothing is queued; the next sync or the sweep picks these up.
  if (it.parse === "done") {
    return { phase: "waiting", label: "Waiting to embed", percent: null };
  }
  if (it.download === "done") {
    return { phase: "waiting", label: sheet ? "Waiting to convert" : "Waiting to parse", percent: null };
  }
  return { phase: "waiting", label: "Waiting to download", percent: null };
}

/** Megabytes to one decimal, the unit an upload's caption counts in. */
export function fmtMb(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1);
}
