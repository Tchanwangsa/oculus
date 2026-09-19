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

export function fmtDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${m.toString().padStart(2, "0")}m`;
  return `${m}m ${s.toString().padStart(2, "0")}s`;
}

/** `m:ss`, or `h:mm:ss` past the hour. `forceHours` keeps a current-time
 *  readout aligned with an over-an-hour total (`0:04:12 / 1:54:46`). */
export function fmtTime(secs: number, forceHours = false): string {
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
    return { text: `${fmtTime(left)} left`, color: "text-warning" };
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

/** Fired when a player changes something the lecture list shows (progress,
 *  a finished download). The panel's player has no list to call back into, so
 *  it says so here and whichever list is mounted refreshes itself. */
export const LECTURES_CHANGED_EVENT = "oculus:lectures-changed";

// ── Chapters ─────────────────────────────────────────────────────────────────

/** Rust's own event (`chapters::app::LECTURE_CHAPTERS_EVENT`), not a window
 *  one: a chaptering run ends in the backend. It is separate from
 *  `LECTURES_CHANGED_EVENT` because that one fires on every playback-progress
 *  save, and a result eight minutes in the making would be lost in it. */
export const LECTURE_CHAPTERS_EVENT = "lecture-chapters";

/** What the event carries: which lecture, how it ended, and why if it failed.
 *  `status` is the `chapter_status` column's own vocabulary. */
export interface ChapterRunFinished {
  lectureId: string;
  status: "ready" | "error";
  chapters: number;
  error: string | null;
}

/** Rust's step report while a run is in flight
 *  (`chapters::app::LECTURE_CHAPTER_PROGRESS_EVENT`). Separate from the finish
 *  above because the two have different lifetimes: a finish is a fact worth
 *  re-reading the database on, a step is a line the panel paints and forgets. */
export const LECTURE_CHAPTER_PROGRESS_EVENT = "lecture-chapter-progress";

/** Which part of the job is running. `agent` and `naming` are both the one
 *  turn — `naming` is the reply arriving, which is the agent having made up
 *  its mind about the whole lecture. */
export type ChapterPhase = "decoding" | "frames" | "agent" | "naming" | "writing";

/** What the run is doing right now. `done`/`total` are only ever set for the
 *  two countable phases; the agent turn has no denominator and is not given a
 *  fake one. `detail` is a tool's own title, `kind` what that tool was. */
export interface ChapterRunProgress {
  lectureId: string;
  phase: ChapterPhase;
  detail: string | null;
  kind: ToolKind | null;
  done: number | null;
  total: number | null;
}

/** The phase, in the panel's words. The detail line underneath says what is
 *  actually being read; this says which of the five stages we are in. */
export const CHAPTER_PHASE_LABEL: Record<ChapterPhase, string> = {
  decoding: "Watching the recording",
  frames: "Grabbing slide frames",
  agent: "Reading the slides",
  naming: "Naming the chapters",
  writing: "Saving chapters",
};

/**
 * Chapter a recording with the agent the `lectureChapters` job is configured
 * with (Settings → AI), the same job `oculus lecture chapters` runs.
 *
 * Returns as soon as the run is claimed — it takes eight to eleven minutes, so
 * nothing waits on it. While it runs the lecture's `chapter_status` is
 * `running`; the end arrives as `LECTURE_CHAPTERS_EVENT`.
 *
 * `source` overrides which of the capture's streams is read. Leave it out and
 * Rust measures it (`chapters::detect`) — which is right often enough that the
 * override exists for the lecture where it is not, and for `--source` on the
 * CLI.
 */
export function findLectureChapters(
  lectureId: string,
  force = false,
  source?: SourceNum
): Promise<void> {
  return invoke("lecture_find_chapters", { lectureId, force, source });
}

// ── Reading copy ─────────────────────────────────────────────────────────────

/** Rust's own event (`reading::app::LECTURE_READING_EVENT`), the reading-copy
 *  job's sibling of `LECTURE_CHAPTERS_EVENT` and separate from it for the
 *  same reason the two jobs are separate: either can finish while the other
 *  has never been run. */
export const LECTURE_READING_EVENT = "lecture-reading";

/** How a reading-copy run ended. `lines` is how many were written — which on
 *  an `error` can still be more than zero, because the job commits window by
 *  window and the ones before the failure are kept (docs/chapters.md). */
export interface ReadingRunFinished {
  lectureId: string;
  status: "ready" | "error";
  lines: number;
  error: string | null;
}

/** Rust's step report while a reading copy is in flight
 *  (`reading::app::LECTURE_READING_PROGRESS_EVENT`). */
export const LECTURE_READING_PROGRESS_EVENT = "lecture-reading-progress";

/** Which part of the reading-copy job is running. There is no `naming` here:
 *  the reply arriving means one window is decided, not the whole lecture, and
 *  the window counter already says that. */
export type ReadingPhase = "decoding" | "frames" | "agent" | "writing";

/**
 * What the reading-copy run is doing right now.
 *
 * `window` is the one thing a chaptering run has no equivalent of, and it is
 * the reason this job can be honest about its progress where the other cannot:
 * a reading copy is a sequence of countable agent turns, so "window 3 of 7" is
 * a real fraction rather than an estimate of a single turn that has not
 * answered.
 */
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
 * Write a reading copy with the agent the `lectureReading` job is configured
 * with (Settings → AI), the same job `oculus lecture reading` runs.
 *
 * Returns as soon as the run is claimed. It needs both the recording *and* the
 * transcript — the copy is a rewrite of the transcript, so none of it is
 * optional reading — and Rust refuses the run outright without them.
 *
 * `source` is `findLectureChapters`': the two jobs decode the same file and
 * choose the stream the same way.
 */
export function writeLectureReading(
  lectureId: string,
  force = false,
  source?: SourceNum
): Promise<void> {
  return invoke("lecture_write_reading", { lectureId, force, source });
}

/** One stream's frame of the moment a dock message carries. */
export interface MomentFrame {
  /** Which stream it came off, 1 and 2 as Echo360 numbers them — the same
   *  numbering the player's picker shows (`SOURCE_LABEL`). */
  source: SourceNum;
  /** Relative to `agents/`, the thread's cwd. */
  path: string;
}

/**
 * One JPEG per downloaded stream of the moment the playhead is at, for a
 * message sent from the player's dock.
 *
 * **Every source on disk, not the one on screen.** Which stream carries the
 * teaching is the theatre's business: a lecturer at the whiteboard leaves
 * source 1 on the room's idle splash for the hour while the derivation is
 * only ever on source 2, so a moment built from the visible pane alone hands
 * the agent a picture of nothing. The message carries every view of that
 * second and the agent reads whichever answers the question.
 *
 * **What comes back are paths the agent reads, not ones the webview can
 * open**: `../lectures/<id>/frames/live/<seconds>-source<n>.jpg`, relative to
 * `agents/`, which every thread runs from. The page never opens the files —
 * it puts the strings in the message and the CLI opens them.
 *
 * ~200 ms per stream: each grab probes a few offsets first, the same defence
 * against a black frame or the room's AV splash that a chaptering run's
 * frames get (docs/chapters.md). A stream that will not decode drops its own
 * line; only a lecture with nothing downloaded is refused.
 */
export function lectureGrabFrames(lectureId: string, seconds: number): Promise<MomentFrame[]> {
  return invoke<MomentFrame[]>("lecture_grab_frames", {
    lectureId,
    seconds: Math.max(0, Math.floor(seconds)),
  });
}

/**
 * Where each chapter ends.
 *
 * **Derived, never stored.** There is no `end_seconds` column: a chapter runs
 * until the next one starts and the last until the lecture does, so the end is
 * arithmetic on two facts that already exist. A stored end would be a second
 * place for the same fact to be wrong — see docs/chapters.md.
 *
 * `duration` is the player's, which is the *element's* where one is loaded:
 * Echo360's catalogue length runs a few seconds short of the file.
 */
export function chapterEnds(starts: number[], duration: number): number[] {
  return starts.map((s, i) => Math.max(s, i + 1 < starts.length ? starts[i + 1] : duration));
}

/**
 * Which span second `t` falls in, or -1 before the first one starts.
 *
 * Named for the shape rather than for chapters: a chapter set and a reading
 * copy's lines are both an ordered list of starts with no ends, and "which one
 * is the playhead in" is the same arithmetic over either. Two copies of it
 * would be two places for an off-by-one to live.
 */
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

/** The standalone full-page player route (panel → expand). `t` titles the tab.
 *  Takes the three columns it reads rather than a whole row, so the ⌘K palette
 *  can route to a lecture from its search hit. */
export function lecturePagePath(
  lec: Pick<Lecture, "id" | "subject_id" | "title">,
): string {
  return `/subjects/${lec.subject_id}/lecture?id=${encodeURIComponent(lec.id)}&t=${encodeURIComponent(lec.title)}`;
}
