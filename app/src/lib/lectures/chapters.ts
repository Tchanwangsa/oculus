import { invoke } from "@tauri-apps/api/core";

import type { SourceNum } from "@/lib/db";
import type { ToolKind } from "@/lib/harness";

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

/** Where each chapter ends — derived, never stored (docs/chapters.md). Pass the
 *  element's `duration`; Echo360's catalogue length runs short. */
export function chapterEnds(starts: number[], duration: number): number[] {
  return starts.map((s, i) => Math.max(s, i + 1 < starts.length ? starts[i + 1] : duration));
}
