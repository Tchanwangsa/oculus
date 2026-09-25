import { invoke } from "@tauri-apps/api/core";

import type { Lecture, SourceNum } from "@/lib/db";
import type { ToolKind } from "@/lib/harness";

// ── VTT parsing ───────────────────────────────────────────────────────────────

export interface Cue {
  start: number;
  end: number;
  text: string;
}

export function parseVtt(vtt: string): Cue[] {
  const cues: Cue[] = [];
  const normalised = vtt.replace(/\r\n/g, "\n");
  const blocks = normalised.split(/\n\n+/);
  for (const block of blocks) {
    const lines = block.trim().split("\n");
    const timeLine = lines.find((l) => l.includes(" --> "));
    if (!timeLine) continue;
    const [startStr, endStr] = timeLine.split(" --> ");
    const start = vttToSecs(startStr?.trim() ?? "");
    const end = vttToSecs(endStr?.split(" ")[0]?.trim() ?? "");
    const text = lines
      .filter((l) => !l.includes(" --> "))
      .map((l) =>
        l
          .replace(/NOTE CONF\s*\{[^}]*\}/g, "")
          .replace(/<[^>]+>/g, "")
          .trim(),
      )
      .join(" ")
      .replace(/^\d+$/, "")
      .trim();
    if (text && start >= 0) cues.push({ start, end, text });
  }
  return cues;
}

function vttToSecs(s: string): number {
  const parts = s.split(":");
  if (parts.length === 3) {
    return Number(parts[0]) * 3600 + Number(parts[1]) * 60 + Number(parts[2]);
  }
  if (parts.length === 2) {
    return Number(parts[0]) * 60 + Number(parts[1]);
  }
  return -1;
}

// ── Formatting ────────────────────────────────────────────────────────────────

export function fmtDurationSecs(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${m.toString().padStart(2, "0")}m`;
  return `${m}m ${s.toString().padStart(2, "0")}s`;
}

/** `m:ss`, or `h:mm:ss` past the hour or with `forceHours`. */
export function fmtClockSecs(secs: number, forceHours = false): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  if (h > 0 || forceHours) {
    return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function fmtLectureDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("en-AU", {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
}

export function progressLabel(lec: Lecture): { text: string; color: string } {
  if (lec.completed) return { text: "Done", color: "text-success" };
  if (lec.progress_seconds > 5) {
    const left = Math.max(0, lec.duration_seconds - lec.progress_seconds);
    return { text: `${fmtClockSecs(left)} left`, color: "text-warning" };
  }
  return { text: "Not watched", color: "text-muted-foreground" };
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

// ── Reading copy ─────────────────────────────────────────────────────────────

/** Tauri event from `reading::app::LECTURE_READING_EVENT`: a run finished. */
export const LECTURE_READING_EVENT = "lecture-reading";

/** Step report (`reading::app::LECTURE_READING_PROGRESS_EVENT`). */
export const LECTURE_READING_PROGRESS_EVENT = "lecture-reading-progress";

export type ReadingPhase = "decoding" | "frames" | "agent" | "writing";

/** As `ChapterRunProgress`, plus `window`: the job's countable agent turns. */
export interface ReadingRunProgress {
  lectureId: string;
  phase: ReadingPhase;
  detail: string | null;
  kind: ToolKind | null;
  done: number | null;
  total: number | null;
  window: { done: number; total: number } | null;
}

export const READING_PHASE_LABEL: Record<ReadingPhase, string> = {
  decoding: "Watching the recording",
  frames: "Grabbing slide frames",
  agent: "Writing the reading copy",
  writing: "Saving lines",
};

/**
 * Start a reading-copy run (the `lectureReading` job, as `oculus lecture
 * reading`). Needs both recording and transcript. `source` as in
 * `findLectureChapters`.
 */
export function writeLectureReading(
  lectureId: string,
  force = false,
  source?: SourceNum
): Promise<void> {
  return invoke("lecture_write_reading", { lectureId, force, source });
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

/** Index of the span `t` falls in over ordered `starts` (chapters or reading
 *  lines), or -1 before the first. */
export function spanAt(starts: number[], t: number): number {
  let idx = -1;
  for (let i = starts.length - 1; i >= 0; i--) {
    if (t >= starts[i]) {
      idx = i;
      break;
    }
  }
  return idx;
}

/** The full-page player route; `t` titles the tab. */
export function lecturePagePath(
  lec: Pick<Lecture, "id" | "subject_id" | "title">,
): string {
  return `/subjects/${lec.subject_id}/lecture?id=${encodeURIComponent(lec.id)}&t=${encodeURIComponent(lec.title)}`;
}
