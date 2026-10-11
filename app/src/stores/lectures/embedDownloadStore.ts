import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getFileByRelativePath, type DbFile } from "@/lib/db";
import { scrapedRow } from "@/lib/files/scrapedRow";

/**
 * Echo360 videos embedded in Canvas pages (`<echo-video>`), resolved and
 * downloaded from their card (`docs/sync.md`). Global and keyed by the
 * embed's `linkId`, so a download outlives the page that started it and two
 * cards for one link agree.
 */

/** `echo360_embed_info`/`echo360_embed_download`'s row. Paths are data-dir-relative. */
export interface EmbedRow {
  linkId: string;
  mediaId: string;
  /** Echo360's media name, e.g. "WWW_WOTWHYWHO_2024.mp4". */
  name: string;
  durationS: number | null;
  restrictDownload: boolean;
  hasCaptions: boolean;
  posterRel: string | null;
  /** The downloaded video; null until downloaded, or once deleted. */
  fileRel: string | null;
  extraSources: number;
}

/** What the commands need to find and launch one embed. */
export interface EmbedRef {
  subjectId: number;
  subjectCode: string;
  linkId: string;
  /** The tag's Canvas retrieve URL. */
  src: string;
}

/** `echo360_embed_download`'s refusal for a `restrictDownload` video, verbatim. */
export const DOWNLOAD_OFF = "Download is turned off for this video";

/** `echo-embed-progress`, exactly as `sync/scrape/embeds/commands.rs` emits it. */
export interface EmbedProgress {
  linkId: string;
  percent: number;
  phase: "downloading" | "muxing" | "complete" | "cancelled" | "error";
}

export type EmbedInfo =
  | { status: "loading" }
  | { status: "ready"; row: EmbedRow }
  | { status: "error"; error: string };

export type EmbedDownload =
  /** `percent` is null until the first byte lands; `muxing` once both streams are in. */
  | { status: "downloading"; percent: number | null; muxing: boolean }
  | { status: "error"; error: string };

interface EmbedsState {
  infos: Record<string, EmbedInfo>;
  downloads: Record<string, EmbedDownload>;
}

export const useEmbeds = create<EmbedsState>(() => ({ infos: {}, downloads: {} }));

function putInfo(linkId: string, info: EmbedInfo) {
  useEmbeds.setState((s) => ({ infos: { ...s.infos, [linkId]: info } }));
}

function putDownload(linkId: string, entry: EmbedDownload | null) {
  useEmbeds.setState((s) => {
    const downloads = { ...s.downloads };
    if (entry) downloads[linkId] = entry;
    else delete downloads[linkId];
    return { downloads };
  });
}

/**
 * Each link's info call. The first one launches the link in Echo360, which
 * records a view, so a link is asked once per app session; a rejection is
 * dropped so the next ask (Retry, a remount) tries again.
 */
const asked = new Map<string, Promise<EmbedRow>>();

/**
 * The embed's row, asked at most once per link. `fresh` asks again, keeping
 * the current row on screen meanwhile; the backend answers that from its
 * cache, so it never launches the link twice.
 */
export function embedInfo(ref: EmbedRef, fresh = false): Promise<EmbedRow> {
  const id = ref.linkId;
  const held = fresh ? undefined : asked.get(id);
  if (held) return held;
  const ask = invoke<EmbedRow>("echo360_embed_info", { ...ref });
  asked.set(id, ask);
  if (useEmbeds.getState().infos[id]?.status !== "ready") putInfo(id, { status: "loading" });
  ask.then(
    (row) => {
      if (asked.get(id) === ask) putInfo(id, { status: "ready", row });
    },
    (e) => {
      if (asked.get(id) !== ask) return;
      asked.delete(id);
      putInfo(id, { status: "error", error: String(e) });
    },
  );
  return ask;
}

function settle(row: EmbedRow) {
  asked.set(row.linkId, Promise.resolve(row));
  putInfo(row.linkId, { status: "ready", row });
}

let watching = false;

/** Subscribed on the first download and kept for the app's lifetime. */
function watchProgress() {
  if (watching) return;
  watching = true;
  void listen<EmbedProgress>("echo-embed-progress", (e) => {
    const { linkId, percent, phase } = e.payload;
    // The invoke settles the rest.
    if (phase !== "downloading" && phase !== "muxing") return;
    const current = useEmbeds.getState().downloads[linkId];
    if (current?.status !== "downloading") return;
    const muxing = phase === "muxing";
    if (current.percent === percent && current.muxing === muxing) return;
    putDownload(linkId, { status: "downloading", percent, muxing });
  }).catch(() => { watching = false; });
}

/**
 * Download one embed into the library. The card flips to Play only once the
 * file's row has landed, so Play always finds it. A failure stays in the store
 * for the card to show and retry; a cancel clears it; a refusal because
 * download is off marks the row instead.
 */
export async function downloadEmbed(ref: EmbedRef): Promise<void> {
  const id = ref.linkId;
  if (useEmbeds.getState().downloads[id]?.status === "downloading") return;
  watchProgress();
  putDownload(id, { status: "downloading", percent: null, muxing: false });
  try {
    const row = await invoke<EmbedRow>("echo360_embed_download", { ...ref });
    if (row.fileRel) await scrapedRow(row.fileRel);
    settle(row);
    putDownload(id, null);
  } catch (e) {
    const error = String(e);
    if (error === DOWNLOAD_OFF) {
      const info = useEmbeds.getState().infos[id];
      if (info?.status === "ready") settle({ ...info.row, restrictDownload: true });
    }
    putDownload(id, error === "cancelled" || error === DOWNLOAD_OFF ? null : { status: "error", error });
  }
}

/** Stop an in-flight download; `downloadEmbed` then settles back to idle. */
export function cancelEmbedDownload(linkId: string): Promise<boolean> {
  return invoke<boolean>("echo360_embed_cancel", { linkId });
}

/**
 * The downloaded video's `files` row, to open. Null when it has gone: the
 * link is asked again, and the backend, finding the file gone, clears it, so
 * the card offers Download.
 */
export async function embedFile(ref: EmbedRef, fileRel: string): Promise<DbFile | null> {
  const file = await getFileByRelativePath(fileRel).catch(() => null);
  if (file) return file;
  await embedInfo(ref, true).catch(() => {});
  return null;
}
