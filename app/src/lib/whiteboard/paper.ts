import type { WhiteboardElement, WhiteboardViewport, Point } from './model';
import type { DrawingKind } from './drawingKinds';
import { WHITEBOARD_ZOOM_LIMITS, clampZoom, type ZoomLimits } from './zoom';

/** What a document opens as: a whiteboard, or a notebook whose pages bound the camera, the pen and the ink. The file's kind (`DrawingKind`). */
export type WhiteboardKind = DrawingKind;

export type PaperTemplate = 'blank' | 'grid' | 'ruled' | 'dotted' | 'graph' | 'cornell' | 'music' | 'isometric';

/** One sheet. Its position is derived from its place in `Paper.pages` (`pageRects`), never stored. */
export interface PaperPage {
  id: string;
  /** Points: the page's size in world units, 1 unit = 1 pt. */
  width: number;
  height: number;
  template?: PaperTemplate;
  [key: string]: unknown;
}

/** A notebook's pages, stacked top to bottom and centred on x = 0. Only a notebook has pages (`notebookPaper`). */
export interface Paper {
  /** World units between one page and the next. */
  gap?: number;
  pages: PaperPage[];
  [key: string]: unknown;
}

export interface PageRect {
  id: string;
  index: number;
  x: number;
  y: number;
  width: number;
  height: number;
  template: PaperTemplate;
}

export const PAPER_TEMPLATES: readonly PaperTemplate[] = ['blank', 'grid', 'ruled', 'dotted', 'graph', 'cornell', 'music', 'isometric'];
export const PAPER_DEFAULTS = { gap: 24, template: 'blank' } as const satisfies { gap: number; template: PaperTemplate };
/** A4 in points (210 x 297 mm). */
export const A4 = { width: 595, height: 842 } as const;
export const MAX_PAGES = 500;
const MAX_SIDE = 14400;
const MAX_GAP = 1000;

const record = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

/** Why `value` is not a paper, or null. `authority/model.rs` `validate_paper` is its port. */
export function paperProblem(value: unknown): string | null {
  if (!record(value)) return 'Drawing paper must be an object.';
  if (value.gap !== undefined && (!finite(value.gap) || value.gap < 0 || value.gap > MAX_GAP)) return 'Drawing paper has an invalid gap.';
  const pages = value.pages;
  if (!Array.isArray(pages) || pages.length === 0 || pages.length > MAX_PAGES) return `Drawing paper needs between 1 and ${MAX_PAGES} pages.`;
  const ids = new Set<string>();
  for (const [i, page] of pages.entries()) {
    if (!record(page) || typeof page.id !== 'string' || !page.id || ids.has(page.id)) return `Drawing page ${i + 1} needs a unique ID.`;
    ids.add(page.id);
    for (const key of ['width', 'height']) {
      const side = page[key];
      if (!finite(side) || side <= 0 || side > MAX_SIDE) return `Drawing page ${i + 1} has an invalid ${key}.`;
    }
    if (page.template !== undefined && !PAPER_TEMPLATES.includes(page.template as PaperTemplate)) return `Drawing page ${i + 1} has an invalid template.`;
  }
  return null;
}

/**
 * The pages `document` is drawn and edited on: its paper when it is a notebook, else nothing. A whiteboard holding `paper`
 * keeps it in its file but is a whiteboard all the same, so every page behaviour asks here rather than reading `paper`.
 */
export const notebookPaper = (document: { paper?: Paper }, notebook: boolean | undefined): Paper | undefined => notebook ? document.paper : undefined;

export const gapOf = (paper: Paper) => paper.gap ?? PAPER_DEFAULTS.gap;
export const templateOf = (page: PaperPage): PaperTemplate => page.template ?? PAPER_DEFAULTS.template;

/** Every page's rectangle in world coordinates, in order: the one layout everything shares. */
export function pageRects(paper: Paper): PageRect[] {
  const gap = gapOf(paper);
  let y = 0;
  return paper.pages.map((page, index) => {
    const rect = { id: page.id, index, x: -page.width / 2, y, width: page.width, height: page.height, template: templateOf(page) };
    y += page.height + gap;
    return rect;
  });
}

/** The rectangle around every page. */
export function paperBounds(paper: Paper): { x: number; y: number; width: number; height: number } {
  const rects = pageRects(paper);
  const left = Math.min(...rects.map(r => r.x)), right = Math.max(...rects.map(r => r.x + r.width));
  const last = rects[rects.length - 1];
  return { x: left, y: 0, width: right - left, height: last.y + last.height };
}

/** The page whose rectangle holds `point`, or the one nearest above it for a point in a gap or off the end. */
export function pageAt(paper: Paper, point: { x: number; y: number }): PageRect {
  const rects = pageRects(paper);
  return rects.find(r => point.y < r.y + r.height + gapOf(paper) / 2) ?? rects[rects.length - 1];
}

/** The page whose rectangle holds `point`, edges included, or null on the desk around and between the pages. */
export function pageContaining(paper: Paper, point: Point): PageRect | null {
  return pageRects(paper).find(r => point.x >= r.x && point.x <= r.x + r.width && point.y >= r.y && point.y <= r.y + r.height) ?? null;
}

/** The point on any page nearest to `point`: `point` itself when it is on a page. */
export function nearestPagePoint(paper: Paper, point: Point): Point {
  let best = point, distance = Infinity;
  for (const r of pageRects(paper)) {
    const near = { x: Math.max(r.x, Math.min(r.x + r.width, point.x)), y: Math.max(r.y, Math.min(r.y + r.height, point.y)) };
    const d = Math.hypot(near.x - point.x, near.y - point.y);
    if (d < distance) { best = near; distance = d; }
    if (!d) break;
  }
  return best;
}

/** `delta` shortened so `centre + delta` stays on a page; another page is fine. */
export function keepOnPage(paper: Paper, centre: Point, delta: Point): Point {
  const target = { x: centre.x + delta.x, y: centre.y + delta.y }, near = nearestPagePoint(paper, target);
  return near === target ? delta : { x: near.x - centre.x, y: near.y - centre.y };
}

/** `pageAt` for many points over one layout: the rectangles are computed once and each page found by binary search. */
export function pageLocator(paper: Paper): (point: Point) => PageRect {
  const rects = pageRects(paper), half = gapOf(paper) / 2;
  return point => {
    let low = 0, high = rects.length - 1;
    while (low < high) {
      const mid = (low + high) >> 1;
      if (point.y < rects[mid].y + rects[mid].height + half) high = mid; else low = mid + 1;
    }
    return rects[low];
  };
}

/** The page an element belongs to: the one holding the centre of its bounds. */
export function pageOfElement(paper: Paper, element: Pick<WhiteboardElement, 'x' | 'y' | 'width' | 'height'>): PageRect {
  return pageAt(paper, { x: element.x + element.width / 2, y: element.y + element.height / 2 });
}

export function createPaper(template: PaperTemplate, orientation: 'portrait' | 'landscape' = 'portrait'): Paper {
  const [width, height] = orientation === 'portrait' ? [A4.width, A4.height] : [A4.height, A4.width];
  return { pages: [{ id: 'p1', width, height, template }] };
}

/** An id no page of `paper` has: `p<n>` for the lowest n above every existing number. */
export function nextPageId(paper: Paper): string {
  const used = paper.pages.map(p => /^p(\d+)$/.exec(p.id)?.[1]).filter((n): n is string => !!n).map(Number);
  return `p${Math.max(0, ...used) + 1}`;
}

/** A new last page the size and template of the one before it. */
export function withAddedPage(paper: Paper): Paper {
  const last = paper.pages[paper.pages.length - 1];
  return { ...paper, pages: [...paper.pages, { ...last, id: nextPageId(paper) }] };
}

/** Screen pixels between the pages and the view's edges at fit, and the most a zoomed-in page may pull away from an edge. */
const PAGE_MARGIN = 24;
/** The share of the view's height that may show desk above the first page or below the last. */
const OVERSCROLL = 0.4;
/** How far below its fitted view a notebook zooms out, so a little desk shows around the page. */
const ZOOM_OUT = 0.6;

type ViewSize = { width: number; height: number };

/** The zoom at which the widest page fills the view's width less a margin each side, uncapped. */
const fitWidthZoom = (paper: Paper, size: ViewSize) => (size.width - PAGE_MARGIN * 2) / Math.max(...paper.pages.map(page => page.width));
/** Fitting the page width, kept between 10% and 140%: what a notebook opens at and what Fit shows. */
const fittedZoom = (paper: Paper, size: ViewSize) => Math.min(1.4, Math.max(WHITEBOARD_ZOOM_LIMITS.min, fitWidthZoom(paper, size)));

/** The camera that shows page `index` from its top, centred, at `zoom` (default: the widest page fills the width, up to 140%). */
export function pageView(paper: Paper, size: ViewSize, index = 0, zoom?: number): { x: number; y: number; zoom: number } {
  const rects = pageRects(paper), rect = rects[Math.min(Math.max(index, 0), rects.length - 1)];
  const scale = zoom ?? fittedZoom(paper, size);
  return { x: size.width / 2, y: PAGE_MARGIN - rect.y * scale, zoom: scale };
}

/** A notebook's zoom range: down to 60% of the fitted view, so some desk always shows around the page, up to 800%. */
export function zoomLimits(paper: Paper, size: ViewSize): ZoomLimits {
  return { min: Math.max(0.01, ZOOM_OUT * fittedZoom(paper, size)), max: WHITEBOARD_ZOOM_LIMITS.max };
}

/**
 * A notebook's camera kept on its pages. Zoom stays in `zoomLimits` (about the view's centre). A page column narrower than
 * the view is centred; a wider one pans only until its edge comes 24px inside the view's. Vertically, desk may show above
 * the first page or below the last for at most 40% of the view's height. Returns `viewport` itself when it is already in.
 */
export function clampCamera(paper: Paper, size: ViewSize, viewport: WhiteboardViewport): WhiteboardViewport {
  if (size.width <= 0 || size.height <= 0) return viewport;
  let { x, y, zoom } = viewport;
  const next = clampZoom(zoom, zoomLimits(paper, size));
  if (next !== zoom) {
    const cx = size.width / 2, cy = size.height / 2;
    x = cx - (cx - x) * next / zoom; y = cy - (cy - y) * next / zoom; zoom = next;
  }
  const bounds = paperBounds(paper), clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
  if (bounds.width * zoom <= size.width - PAGE_MARGIN * 2) x = size.width / 2 - (bounds.x + bounds.width / 2) * zoom;
  else x = clamp(x, size.width - PAGE_MARGIN - (bounds.x + bounds.width) * zoom, PAGE_MARGIN - bounds.x * zoom);
  const highest = Math.max(PAGE_MARGIN, OVERSCROLL * size.height) - bounds.y * zoom;
  const lowest = Math.min(size.height - PAGE_MARGIN, (1 - OVERSCROLL) * size.height) - (bounds.y + bounds.height) * zoom;
  // Pages shorter than the band between those limits sit in the middle of the view.
  y = lowest <= highest ? clamp(y, lowest, highest) : (size.height - bounds.height * zoom) / 2 - bounds.y * zoom;
  return x === viewport.x && y === viewport.y && zoom === viewport.zoom ? viewport : { ...viewport, x, y, zoom };
}

/** The page at the centre of the view. */
export function pageInView(paper: Paper, viewport: WhiteboardViewport, size: ViewSize): PageRect {
  return pageAt(paper, { x: (size.width / 2 - viewport.x) / viewport.zoom, y: (size.height / 2 - viewport.y) / viewport.zoom });
}

/** The centre of the part of `rect` the view shows, or of the whole page when none of it is on screen. */
export function visiblePageCentre(rect: PageRect, viewport: WhiteboardViewport, size: ViewSize): Point {
  const left = Math.max(rect.x, -viewport.x / viewport.zoom), right = Math.min(rect.x + rect.width, (size.width - viewport.x) / viewport.zoom);
  const top = Math.max(rect.y, -viewport.y / viewport.zoom), bottom = Math.min(rect.y + rect.height, (size.height - viewport.y) / viewport.zoom);
  return left < right && top < bottom ? { x: (left + right) / 2, y: (top + bottom) / 2 } : { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
}

/** Elements the pointer can pick: locked ones (an assignment's page images) are backdrops, never selected, moved or erased. */
export const isLocked = (element: WhiteboardElement) => element.locked === true;

const selectableCache = new WeakMap<readonly WhiteboardElement[], readonly WhiteboardElement[]>();
/** `elements` without the locked ones; the same array when none is locked, so a whiteboard's cached geometry is reused. */
export function selectableElements(elements: readonly WhiteboardElement[]): readonly WhiteboardElement[] {
  const cached = selectableCache.get(elements);
  if (cached) return cached;
  const result = elements.some(isLocked) ? elements.filter(element => !isLocked(element)) : elements;
  selectableCache.set(elements, result);
  return result;
}
