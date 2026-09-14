import { invoke } from "@tauri-apps/api/core";

import type { Lecture, SourceNum } from "@/lib/db";

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

/**
 * Chapter a recording with the agent the `lectureChapters` job is configured
 * with (Settings → AI), the same job `oculus lecture chapters` runs.
 *
 * Returns as soon as the run is claimed — it takes eight to eleven minutes, so
 * nothing waits on it. While it runs the lecture's `chapter_status` is
 * `running`; the end arrives as `LECTURE_CHAPTERS_EVENT`.
 */
export function findLectureChapters(lectureId: string, force = false): Promise<void> {
  return invoke("lecture_find_chapters", { lectureId, force });
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

/** Which chapter second `t` falls in, or -1 before the first one starts. */
export function chapterAt(starts: number[], t: number): number {
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
