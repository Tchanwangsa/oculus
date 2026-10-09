import type { PdfPageSize } from "@/lib/pdfView";

/**
 * Where the viewer puts its pages, as plain arithmetic over the page sizes
 * `pdf_open` reports: every scroll offset, page range, fit and zoom anchor is
 * read from this model rather than measured, since page zoom scales
 * `getBoundingClientRect` (docs/ui.md).
 */

export type LayoutMode = "scroll" | "single" | "spread";

/** CSS pixels per point at 100%: a page shows at its printed size. */
export const UNIT = 96 / 72;

/** The gutter around the pages and the gap between two. */
export const PAD = 16;
export const GAP = 16;

export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 10;

/** The fit caps here; a page-width fit blows a slide past the screen. */
const MAX_FIT = 1.25;

export interface Box {
  page: number;
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface Layout {
  boxes: Box[];
  width: number;
  height: number;
}

/** The pages the toolbar names: one page, a spread, or a scrolled range. */
export interface Shown {
  first: number;
  last: number;
}

export const clampZoom = (s: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, s));

/** Spreads pair 1|2, 3|4, so a spread starts on an odd page. */
export const spreadStart = (page: number) => (page % 2 === 0 ? page - 1 : page);

/** The pages a paged layout shows with `page` in view. */
export function pagesAround(page: number, count: number, mode: LayoutMode): number[] {
  if (mode !== "spread") return [page];
  const first = spreadStart(page);
  return first + 1 <= count ? [first, first + 1] : [first];
}

/** Continuous scroll stacks every page; the paged layouts hold only `current`'s
 *  page or spread. Narrower pages centre in the wider of the view and the
 *  widest page. */
export function layoutPages(
  sizes: readonly PdfPageSize[],
  mode: LayoutMode,
  scale: number,
  current: number,
  viewWidth: number,
): Layout {
  const k = scale * UNIT;
  if (mode === "scroll") {
    const widest = sizes.reduce((w, s) => Math.max(w, s.width * k), 0);
    const width = Math.max(viewWidth, widest + 2 * PAD);
    let top = PAD;
    const boxes = sizes.map((s, i) => {
      const box = { page: i + 1, left: (width - s.width * k) / 2, top, width: s.width * k, height: s.height * k };
      top += box.height + GAP;
      return box;
    });
    return { boxes, width, height: top - GAP + PAD };
  }
  const pages = pagesAround(current, sizes.length, mode);
  const row = pages.reduce((w, p) => w + sizes[p - 1].width * k, 0) + GAP * (pages.length - 1);
  const tallest = pages.reduce((h, p) => Math.max(h, sizes[p - 1].height * k), 0);
  const width = Math.max(viewWidth, row + 2 * PAD);
  let left = (width - row) / 2;
  const boxes = pages.map((p) => {
    const s = sizes[p - 1];
    const box = { page: p, left, top: PAD, width: s.width * k, height: s.height * k };
    left += box.width + GAP;
    return box;
  });
  return { boxes, width, height: tallest + 2 * PAD };
}

/** The fit on open and on reset: a portrait page fills the width, anything
 *  wider fits whole, capped at `MAX_FIT`. A spread fits both its pages. */
export function fitScale(
  sizes: readonly PdfPageSize[],
  mode: LayoutMode,
  page: number,
  viewWidth: number,
  viewHeight: number,
): number {
  const pages = pagesAround(page, sizes.length, mode).map((p) => sizes[p - 1]);
  const row = pages.reduce((w, s) => w + s.width, 0);
  const tallest = pages.reduce((h, s) => Math.max(h, s.height), 0);
  if (!row || !tallest) return 1;
  const byWidth = (viewWidth - 2 * PAD - GAP * (pages.length - 1)) / (row * UNIT);
  const byHeight = (viewHeight - 2 * PAD) / (tallest * UNIT);
  const portrait = pages[0].width <= pages[0].height;
  return clampZoom(Math.min(MAX_FIT, portrait ? byWidth : Math.min(byWidth, byHeight)));
}

/** How much of `box` shows in the band `top..top + height`. */
const seen = (box: Box, top: number, height: number) =>
  Math.min(box.top + box.height, top + height) - Math.max(box.top, top);

/** In continuous scroll, a page counts toward the toolbar's range when it
 *  fills this share of the view — a sliver at an edge is not being read — or
 *  shows this share of itself, which catches small pages zoomed out. */
const VIEW_SHARE = 0.2;
const PAGE_SHARE = 0.5;

/** The range the toolbar names and the page that fills the most of the view. */
export function shownPages(layout: Layout, top: number, height: number): { shown: Shown; main: number } | null {
  let first = Infinity;
  let last = -Infinity;
  let main = 0;
  let most = 0;
  for (const box of layout.boxes) {
    if (box.top > top + height) break;
    const s = seen(box, top, height);
    if (s <= 0) continue;
    if (s >= height * VIEW_SHARE || s >= box.height * PAGE_SHARE) {
      first = Math.min(first, box.page);
      last = Math.max(last, box.page);
    }
    if (s > most) {
      most = s;
      main = box.page;
    }
  }
  if (!main) return null;
  if (first === Infinity) first = last = main;
  return { shown: { first, last }, main };
}

/** The pages worth drawing: on screen, or within one view's height of it. */
export function nearPages(layout: Layout, top: number, height: number): Shown {
  let first = 0;
  let last = 0;
  for (const box of layout.boxes) {
    if (box.top > top + 2 * height) break;
    if (seen(box, top - height, 3 * height) <= 0) continue;
    if (!first) first = box.page;
    last = box.page;
  }
  return { first, last };
}

/** A view point as a page and a spot on it in fractions of its size, so a
 *  zoom can put the same spot back under the pointer. */
export interface Anchor {
  page: number;
  fx: number;
  fy: number;
  /** The point, in the view. */
  vx: number;
  vy: number;
}

export function anchorAt(layout: Layout, x: number, y: number, vx: number, vy: number): Anchor | null {
  let best: Box | null = null;
  let distance = Infinity;
  for (const box of layout.boxes) {
    const d = y < box.top ? box.top - y : y > box.top + box.height ? y - box.top - box.height : 0;
    if (d < distance) {
      distance = d;
      best = box;
    }
  }
  if (!best) return null;
  return { page: best.page, fx: (x - best.left) / best.width, fy: (y - best.top) / best.height, vx, vy };
}
