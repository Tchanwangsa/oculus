import { readWhiteboard } from "@/lib/whiteboard/files";
import { notebookCover } from "@/lib/drive/listing";
import { parseWhiteboardDocument } from "@/lib/whiteboard/model";
import { pageRects } from "@/lib/whiteboard/paper";
import { exportWhiteboardPng, renderPagePng } from "@/lib/whiteboard/render";
import { PAGE_COLOR } from "@/lib/whiteboard/renderPaper";
import { createJobQueue, PICTURE_WIDTH, PictureCache, stampKey, type FileStamp } from "./pictureCache";

/** A drawing's picture: a blob URL and the colour behind it. */
export interface DrawnPicture { url: string; background: string }

/** Reading, parsing and painting all run on the main thread (a drawing can be
 *  megabytes of JSON with its images inline), so one drawing at a time. */
const draw = createJobQueue(1);

/** About 60 KB a picture at most, so a few MB in all. */
const drawings = new PictureCache<DrawnPicture | null>(150, (picture) => {
  if (picture) URL.revokeObjectURL(picture.url);
});

async function drawFile(path: string, notebook: boolean): Promise<DrawnPicture | null> {
  const document = parseWhiteboardDocument(await readWhiteboard(path));
  let blob: Blob | null;
  let background = document.background;
  if (notebook) {
    const first = document.paper && pageRects(document.paper)[0];
    if (!first) return null;
    blob = await renderPagePng(document, first.id, PICTURE_WIDTH / first.width);
    background = PAGE_COLOR;
  } else {
    blob = await exportWhiteboardPng(document, { maxSize: PICTURE_WIDTH });
  }
  return blob ? { url: URL.createObjectURL(blob), background } : null;
}

/**
 * A `.whiteboard` fitted to its elements' bounds, or a notebook's first page,
 * rendered from the file by the editor's own renderer; null for an empty
 * whiteboard. Kept until the file changes.
 */
export function drawnPicture(file: FileStamp, notebook: boolean, live: () => boolean): Promise<DrawnPicture | null> {
  return drawings.load(stampKey(file), (anyLive) => draw(() => drawFile(file.path, notebook), anyLive), live);
}

/** A notebook's cover: the first-page picture its editor writes for chat
 *  agents (`drive_notebook_cover`), and when it was read. */
export interface Cover { url: string; read: number }

/** The editor redraws a notebook's pictures a moment after the file is saved,
 *  so a cover read sooner than this after the save may be the old page. */
const COVER_SETTLE_MS = 5000;

const covers = new PictureCache<Cover>(150, (cover) => URL.revokeObjectURL(cover.url));

export const cachedCover = (fileId: number) => covers.peek(String(fileId));

/** True when `cover` was read after the notebook's last save had settled. */
export const coverIsFresh = (cover: Cover, modified: number | null) =>
  modified == null || cover.read > modified + COVER_SETTLE_MS;

/** Reads the notebook's cover and keeps it; null when it has none yet. */
export async function readCover(fileId: number): Promise<Cover | null> {
  const blob = await notebookCover(fileId);
  if (!blob) return null;
  const cover = { url: URL.createObjectURL(blob), read: Date.now() };
  covers.set(String(fileId), cover);
  return cover;
}

/** What a drawing's card can show in its first frame: a kept cover, else a
 *  kept rendering; null when neither has been made yet (or it was empty). */
export function cachedDrawnPicture(file: FileStamp, notebook: boolean, fileId?: number): DrawnPicture | null {
  const cover = notebook && fileId != null ? cachedCover(fileId) : undefined;
  if (cover) return { url: cover.url, background: PAGE_COLOR };
  return drawings.peek(stampKey(file)) ?? null;
}
