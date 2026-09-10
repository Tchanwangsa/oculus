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

/** The standalone full-page player route (peek → expand). `t` titles the tab. */
export function lecturePagePath(lec: Lecture): string {
  return `/subjects/${lec.subject_id}/lecture?id=${encodeURIComponent(lec.id)}&t=${encodeURIComponent(lec.title)}`;
}
