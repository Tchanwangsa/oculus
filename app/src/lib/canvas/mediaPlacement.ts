import type { CanvasDocument, CanvasElement, CanvasViewport, Point } from './model';
import { notebookPaper, pageContaining, pageInView, pageRects, visiblePageCentre, type Paper } from './paper';
import { secureId } from './ids';

export interface CanvasImageSource { dataUrl: string; width: number; height: number; name: string }
type ViewSize = { width: number; height: number };

const GAP = 24;

/** Each image's size: no larger than the view comfortably shows, and in a notebook no larger than its page. */
function imageBoxes(images: readonly CanvasImageSource[], viewport: CanvasViewport, size: ViewSize, page?: { width: number; height: number }) {
  const maxWidth = Math.min(480, Math.max(160, (size.width - 96) / viewport.zoom), page?.width ?? Infinity);
  const maxHeight = Math.min(600, Math.max(160, (size.height - 180) / viewport.zoom), page?.height ?? Infinity);
  return images.map(image => {
    if (!Number.isFinite(image.width) || !Number.isFinite(image.height) || image.width <= 0 || image.height <= 0) throw new Error('An imported image has invalid dimensions.');
    const scale = Math.min(1, maxWidth / image.width, maxHeight / image.height);
    return { width: image.width * scale, height: image.height * scale };
  });
}

const imageElement = (image: CanvasImageSource, box: { width: number; height: number }, x: number, y: number): CanvasElement => ({
  id: secureId(), type: 'image', x, y, ...box, dataUrl: image.dataUrl, sourceName: image.name,
  naturalWidth: image.width, naturalHeight: image.height,
  stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0,
});

/** Stack a batch in reading order, centring its first image at `at` or the view centre. */
export function placeCanvasImages(images: readonly CanvasImageSource[], viewport: CanvasViewport, size: ViewSize, at?: Point): CanvasElement[] {
  if (!images.length) return [];
  const boxes = imageBoxes(images, viewport, size);
  const centre = at?.x ?? (size.width / 2 - viewport.x) / viewport.zoom;
  let top = (at?.y ?? (size.height / 2 - viewport.y) / viewport.zoom) - boxes[0].height / 2;
  return images.map((image, index) => {
    const box = boxes[index];
    const y = top;
    top += box.height + GAP;
    return imageElement(image, box, centre - box.width / 2, y);
  });
}

/**
 * A notebook's batch: onto the page under `at`, or else the page in view, centred on `at` or the middle of the page's
 * visible part. Each image fits its page and stays inside it; one that no longer fits below the last moves to the top
 * of the next page, and past the last page it keeps to the bottom of that one.
 */
export function placeNotebookImages(images: readonly CanvasImageSource[], paper: Paper, viewport: CanvasViewport, size: ViewSize, at?: Point): CanvasElement[] {
  if (!images.length) return [];
  const rects = pageRects(paper), dropped = at && pageContaining(paper, at);
  let page = rects[(dropped ?? pageInView(paper, viewport, size)).index];
  const anchor = dropped ? at : visiblePageCentre(page, viewport, size);
  const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
  let top: number | null = null;
  return images.map(image => {
    let box = imageBoxes([image], viewport, size, page)[0];
    if (top === null) top = anchor.y - box.height / 2;
    else if (top + box.height > page.y + page.height && page.index + 1 < rects.length) {
      page = rects[page.index + 1];
      box = imageBoxes([image], viewport, size, page)[0];
      top = page.y + Math.min(GAP, page.height - box.height);
    }
    const y = clamp(top, page.y, page.y + page.height - box.height);
    const x = clamp(anchor.x - box.width / 2, page.x, page.x + page.width - box.width);
    top = y + box.height + GAP;
    return imageElement(image, box, x, y);
  });
}

/** Where `insertImages` puts a batch: onto a notebook's pages, or the plain stack on a canvas. */
export function placeImagesIn(document: CanvasDocument, notebook: boolean, images: readonly CanvasImageSource[], size: ViewSize, at?: Point): CanvasElement[] {
  const paper = notebookPaper(document, notebook);
  return paper ? placeNotebookImages(images, paper, document.viewport, size, at) : placeCanvasImages(images, document.viewport, size, at);
}
