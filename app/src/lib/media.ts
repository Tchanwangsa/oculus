import { invoke } from "@tauri-apps/api/core";
import { loadDataDir } from "@/hooks/useDataDir";

/** The media player's plumbing (`components/media/`): where a video streams
 *  from, its WebVTT cues, and the clock the player and its lists read. */

interface MediaServerInfo {
  port: number;
  token: string;
}

let infoPromise: Promise<MediaServerInfo> | null = null;

/**
 * URL for a library video via the localhost server in `media.rs`, from an
 * absolute path or one relative to the data dir. WebKit (macOS 26) rejects
 * media on custom schemes like `convertFileSrc`'s with
 * MEDIA_ERR_SRC_NOT_SUPPORTED.
 */
export async function mediaSrc(path: string): Promise<string> {
  infoPromise ??= invoke<MediaServerInfo>("media_server_info");
  const absolute = path.startsWith("/") ? path : `${await loadDataDir()}/${path}`;
  const { port, token } = await infoPromise;
  return `http://127.0.0.1:${port}/${token}?path=${encodeURIComponent(absolute)}`;
}

// ── One sound at a time ───────────────────────────────────────────────────────

/** Library videos' own elements (`VideoFileViewer`), so a lecture starting
 *  can silence them; the lecture's are `lib/lecturePlayback.ts`'s. */
const libraryVideos = new Set<HTMLVideoElement>();

export function registerLibraryVideo(v: HTMLVideoElement): () => void {
  libraryVideos.add(v);
  return () => void libraryVideos.delete(v);
}

/** Pause every library video but `except`. */
export function pauseLibraryVideos(except?: HTMLVideoElement) {
  for (const v of libraryVideos) if (v !== except && !v.paused) v.pause();
}

// ── WebVTT ────────────────────────────────────────────────────────────────────

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

// ── Clock ─────────────────────────────────────────────────────────────────────

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

/** Index of the span `t` falls in over ordered `starts` (cues or chapters),
 *  or -1 before the first. */
export function spanAt(starts: number[], t: number): number {
  if (Number.isNaN(t)) return -1;
  // Starts are chronological; the upper bound also selects the last duplicate.
  let low = 0;
  let high = starts.length;
  while (low < high) {
    const middle = low + Math.floor((high - low) / 2);
    if (starts[middle] <= t) low = middle + 1;
    else high = middle;
  }
  return low - 1;
}
