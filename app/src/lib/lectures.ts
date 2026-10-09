import { invoke } from "@tauri-apps/api/core";

import type { Lecture, SourceNum } from "@/lib/db";
import type { ToolKind } from "@/lib/harness";

// ── Formatting ────────────────────────────────────────────────────────────────

export function fmtDurationSecs(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${m.toString().padStart(2, "0")}m`;
  return `${m}m ${s.toString().padStart(2, "0")}s`;
}

export function fmtLectureDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("en-AU", {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
}

// ── Download progress event payload ───────────────────────────────────────────

export interface DlProgress {
  mediaId: string;
  /** Which stream this bar belongs to; see `dlKey` in the download store. */
  source: SourceNum;
  percent: number;
  phase: string;
}

/** Window event: a player changed something the mounted lecture list shows. */
export const LECTURES_CHANGED_EVENT = "oculus:lectures-changed";

// ── Chapters ─────────────────────────────────────────────────────────────────

/** Tauri event from `chapters::app::LECTURE_CHAPTERS_EVENT`: a run finished. */
export const LECTURE_CHAPTERS_EVENT = "lecture-chapters";

/** Step report while a run is in flight (`chapters::app::LECTURE_CHAPTER_PROGRESS_EVENT`). */
export const LECTURE_CHAPTER_PROGRESS_EVENT = "lecture-chapter-progress";

/** `agent` and `naming` are one turn; `naming` is its reply arriving. */
export type ChapterPhase = "decoding" | "frames" | "agent" | "naming" | "writing";

/** `done`/`total` are set only for countable phases, never the agent turn.
 *  `detail` is a tool's title, `kind` its kind. */
export interface ChapterRunProgress {
  lectureId: string;
  phase: ChapterPhase;
  detail: string | null;
  kind: ToolKind | null;
  done: number | null;
  total: number | null;
}

export const CHAPTER_PHASE_LABEL: Record<ChapterPhase, string> = {
  decoding: "Watching the recording",
  frames: "Grabbing slide frames",
  agent: "Reading the slides",
  naming: "Naming the chapters",
  writing: "Saving chapters",
};

/**
 * Start a chaptering run (the `lectureChapters` job, as `oculus lecture
 * chapters`). Returns once claimed; the end arrives as `LECTURE_CHAPTERS_EVENT`.
 * `source` overrides the stream Rust would detect (`chapters::detect`).
 */
export function findLectureChapters(
  lectureId: string,
  force = false,
  source?: SourceNum
): Promise<void> {
  return invoke("lecture_find_chapters", { lectureId, force, source });
}

// ── Where the content ends ───────────────────────────────────────────────────

/** Tauri event from `lecture_end::app::LECTURE_END_EVENT`: a run finished. */
export const LECTURE_END_EVENT = "lecture-end";

/** `seconds` and `quote` are set only when `status` is `ready`. */
export interface LectureEndFinished {
  lectureId: string;
  status: "ready" | "none" | "error";
  seconds: number | null;
  quote: string | null;
  error: string | null;
}

/**
 * Find where a lecture's content ends (the `lectureEnd` job, as `oculus
 * lecture end`). Needs the transcript only. Returns once claimed; the end
 * arrives as `LECTURE_END_EVENT`.
 */
export function findLectureEnd(lectureId: string, force = false): Promise<void> {
  return invoke("lecture_find_end", { lectureId, force });
}

export interface MomentFrame {
  source: SourceNum;
  /** Relative to `agents/`, the thread's cwd. */
  path: string;
}

/**
 * One JPEG per *downloaded* stream at `seconds`, for a dock message — every
 * source, not just the visible one, since either may carry the teaching.
 * Returns `agents/`-relative paths for the agent, not the webview.
 */
export function lectureGrabFrames(lectureId: string, seconds: number): Promise<MomentFrame[]> {
  return invoke<MomentFrame[]>("lecture_grab_frames", {
    lectureId,
    seconds: Math.max(0, Math.floor(seconds)),
  });
}

/** Where each chapter ends — derived, never stored (docs/chapters.md). Pass the
 *  element's `duration`; Echo360's catalogue length runs short. */
export function chapterEnds(starts: number[], duration: number): number[] {
  return starts.map((s, i) => Math.max(s, i + 1 < starts.length ? starts[i + 1] : duration));
}

/** The full-page player route; `t` titles the tab. */
export function lecturePagePath(
  lec: Pick<Lecture, "id" | "subject_id" | "title">,
): string {
  return `/subjects/${lec.subject_id}/lecture?id=${encodeURIComponent(lec.id)}&t=${encodeURIComponent(lec.title)}`;
}

/** The lecture id a `lecturePagePath` route names, or null for any other path. */
export function lecturePageId(path: string | null | undefined): string | null {
  const [pathname, search = ""] = (path ?? "").split("?");
  if (!/^\/subjects\/\d+\/lecture$/.test(pathname)) return null;
  return new URLSearchParams(search).get("id");
}
