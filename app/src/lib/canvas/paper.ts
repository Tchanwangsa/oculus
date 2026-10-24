import type { CanvasElement } from './model';

export type PaperTemplate = 'blank' | 'grid' | 'ruled' | 'dotted';

/** One sheet. Its position is derived from its place in `Paper.pages` (`pageRects`), never stored. */
export interface PaperPage {
  id: string;
  /** Points: the page's size in world units, 1 unit = 1 pt. */
  width: number;
  height: number;
  template?: PaperTemplate;
  [key: string]: unknown;
}

/** A notebook's pages, stacked top to bottom and centred on x = 0. A document without it is a whiteboard. */
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

export const PAPER_TEMPLATES: readonly PaperTemplate[] = ['blank', 'grid', 'ruled', 'dotted'];
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
  if (!record(value)) return 'Canvas paper must be an object.';
  if (value.gap !== undefined && (!finite(value.gap) || value.gap < 0 || value.gap > MAX_GAP)) return 'Canvas paper has an invalid gap.';
  const pages = value.pages;
  if (!Array.isArray(pages) || pages.length === 0 || pages.length > MAX_PAGES) return `Canvas paper needs between 1 and ${MAX_PAGES} pages.`;
  const ids = new Set<string>();
  for (const [i, page] of pages.entries()) {
    if (!record(page) || typeof page.id !== 'string' || !page.id || ids.has(page.id)) return `Canvas page ${i + 1} needs a unique ID.`;
    ids.add(page.id);
    for (const key of ['width', 'height']) {
      const side = page[key];
      if (!finite(side) || side <= 0 || side > MAX_SIDE) return `Canvas page ${i + 1} has an invalid ${key}.`;
    }
    if (page.template !== undefined && !PAPER_TEMPLATES.includes(page.template as PaperTemplate)) return `Canvas page ${i + 1} has an invalid template.`;
  }
  return null;
}

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

/** The page an element belongs to: the one holding the centre of its bounds. */
export function pageOfElement(paper: Paper, element: Pick<CanvasElement, 'x' | 'y' | 'width' | 'height'>): PageRect {
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

/** The camera that shows page `index` from its top, centred, at `zoom` (default: the widest page fills the width, up to 140%). */
export function pageView(paper: Paper, size: { width: number; height: number }, index = 0, zoom?: number): { x: number; y: number; zoom: number } {
  const rects = pageRects(paper), rect = rects[Math.min(Math.max(index, 0), rects.length - 1)];
  const fitted = Math.min(1.4, Math.max(0.1, (size.width - 48) / Math.max(...rects.map(r => r.width))));
  const scale = zoom ?? fitted;
  return { x: size.width / 2, y: 24 - rect.y * scale, zoom: scale };
}

/** Elements the pointer can pick: locked ones (an assignment's page images) are backdrops, never selected, moved or erased. */
export const isLocked = (element: CanvasElement) => element.locked === true;

const selectableCache = new WeakMap<readonly CanvasElement[], readonly CanvasElement[]>();
/** `elements` without the locked ones; the same array when none is locked, so a whiteboard's cached geometry is reused. */
export function selectableElements(elements: readonly CanvasElement[]): readonly CanvasElement[] {
  const cached = selectableCache.get(elements);
  if (cached) return cached;
  const result = elements.some(isLocked) ? elements.filter(element => !isLocked(element)) : elements;
  selectableCache.set(elements, result);
  return result;
}
