import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getFileByRelativePath, type DbFile } from "@/lib/db";
import { FILE_SCRAPED_EVENT } from "@/lib/syncRunner";

/**
 * On-demand downloads of module videos, which a sync lists but never fetches
 * (`docs/sync.md`). Global so a download outlives the Modules page that
 * started it. Keyed by Canvas file id.
 */

/** `canvas-video-progress`, exactly as `scrape.rs` emits it. */
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
 * The row `useBackendEvents` writes from the download's `scrape-file` event.
 * The command can resolve before that write lands, so wait for its
 * FILE_SCRAPED_EVENT; the listener goes on first so the event can't slip by.
 */
function rowFor(relativePath: string): Promise<DbFile | null> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (file: DbFile | null) => {
      if (settled) return;
      settled = true;
      window.removeEventListener(FILE_SCRAPED_EVENT, check);
      clearTimeout(fallback);
      resolve(file);
    };
    const check = () => {
      getFileByRelativePath(relativePath)
        .then((file) => { if (file) finish(file); })
        .catch(() => {});
    };
    window.addEventListener(FILE_SCRAPED_EVENT, check);
    // A row write that failed never raises the event; stop waiting for it.
    const fallback = setTimeout(() => {
      getFileByRelativePath(relativePath).then(finish, () => finish(null));
    }, 10_000);
    check();
  });
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
    const file = await rowFor(rel);
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
