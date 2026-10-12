import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DbFile } from "@/lib/db";
import { scrapedRow } from "@/lib/files/scrapedRow";

/**
 * On-demand downloads of Canvas videos — module videos and a page's
 * `<canvas-video>` — which a sync never fetches (`docs/sync.md`). Global so a
 * download outlives the page that started it. Keyed by Canvas file id.
 */

/** `canvas-video-progress`, exactly as `sync/scrape/videos.rs` emits it. */
export interface VideoProgress {
  canvasFileId: number;
  percent: number;
  phase: "downloading" | "complete" | "error" | "cancelled";
}

export type VideoDownload =
  /** `percent` is null until the first byte lands. */
  | { status: "downloading"; percent: number | null }
  | { status: "error"; error: string }
  /** The file's row, so every Modules pane sees it before its list reloads. */
  | { status: "done"; file: DbFile };

interface VideoDownloadsState {
  downloads: Record<number, VideoDownload>;
}

export const useVideoDownloads = create<VideoDownloadsState>(() => ({
  downloads: {},
}));

function put(id: number, entry: VideoDownload | null) {
  useVideoDownloads.setState((s) => {
    const downloads = { ...s.downloads };
    if (entry) downloads[id] = entry;
    else delete downloads[id];
    return { downloads };
  });
}

let watching = false;

/** Subscribed on the first download and kept for the app's lifetime. */
function watchProgress() {
  if (watching) return;
  watching = true;
  void listen<VideoProgress>("canvas-video-progress", (e) => {
    const { canvasFileId, percent, phase } = e.payload;
    if (phase !== "downloading") return; // the invoke settles the rest
    const current = useVideoDownloads.getState().downloads[canvasFileId];
    if (current?.status !== "downloading" || current.percent === percent) return;
    put(canvasFileId, { status: "downloading", percent });
  }).catch(() => { watching = false; });
}

/**
 * Download one module video into the library and resolve to its file row.
 * Resolves null when it was already running, was cancelled, or failed — a
 * failure stays in the store for the row to show and retry.
 */
export async function downloadVideo(
  subject: { id: number; code: string },
  canvasFileId: number,
): Promise<DbFile | null> {
  if (useVideoDownloads.getState().downloads[canvasFileId]?.status === "downloading") {
    return null;
  }
  watchProgress();
  put(canvasFileId, { status: "downloading", percent: null });
  try {
    const rel = await invoke<string>("canvas_download_video", {
      subjectId: subject.id,
      subjectCode: subject.code,
      canvasFileId,
    });
    const file = await scrapedRow(rel);
    put(canvasFileId, file ? { status: "done", file } : null);
    return file;
  } catch (e) {
    const error = String(e);
    put(canvasFileId, error.includes("cancelled") ? null : { status: "error", error });
    return null;
  }
}

/** Stop an in-flight download; `downloadVideo` then resolves null. */
export function cancelVideoDownload(canvasFileId: number): Promise<boolean> {
  return invoke<boolean>("canvas_cancel_video", { canvasFileId });
}
