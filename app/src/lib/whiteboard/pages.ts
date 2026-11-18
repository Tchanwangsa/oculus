/**
 * Page operations on a notebook. Each returns a new document (or the same one when nothing changes) whose elements
 * moved with their pages: an element belongs to the page holding the centre of its drawn bounds (`elementPage`, the
 * page the screen clips it to), and keeps its offset from that page's top-centre wherever the page ends up.
 */
import type { WhiteboardDocument, WhiteboardElement } from './model';
import { isLocked, nextPageId, pageRects, templateOf, withAddedPage, type PageRect, type Paper, type PaperPage, type PaperTemplate } from './paper';
import { elementPage } from './renderPaper';
import { elementBounds } from './geometry';
import { cloneWhiteboardElements } from './renderClipboard';
import { secureId } from './ids';
import type { WhiteboardImportedImage } from './importMedia';

export type PageSize = { width: number; height: number };

const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
const withPages = (paper: Paper, pages: PaperPage[]): Paper => ({ ...paper, pages });
const valid = (paper: Paper | undefined, index: number): paper is Paper => !!paper && Number.isInteger(index) && index >= 0 && index < paper.pages.length;
/** `pages` with `added` at `at`. */
const spliced = (pages: readonly PaperPage[], at: number, ...added: PaperPage[]) => [...pages.slice(0, at), ...added, ...pages.slice(at)];
const replaced = (pages: readonly PaperPage[], index: number, page: PaperPage) => pages.map((old, i) => i === index ? page : old);

/**
 * `document` laid out on `paper`: every element follows its page (matched by id) to that page's new place; elements
 * whose page is gone are dropped. Elements of page `keepOn` whose centre would fall off it (a page made smaller) are
 * moved just enough to keep their centre, and so their page, with the rest clipped. `added` go in last: locked ones
 * above the existing locked backdrops, the rest on top.
 */
function relayout(document: WhiteboardDocument, paper: Paper, added: readonly WhiteboardElement[] = [], keepOn?: string): WhiteboardDocument {
  const old = document.paper!;
  const places = new Map(pageRects(paper).map(rect => [rect.id, rect]));
  const elements: WhiteboardElement[] = [];
  for (const element of document.elements) {
    const from = elementPage(old, element), to = places.get(from.id);
    if (!to) continue;
    let dx = 0, dy = to.y - from.y;
    if (to.id === keepOn) {
      const b = elementBounds(element), cx = b.x + b.width / 2, cy = b.y + b.height / 2 + dy;
      dx = clamp(cx, to.x, to.x + to.width) - cx;
      dy += clamp(cy, to.y, to.y + to.height) - cy;
    }
    elements.push(dx || dy ? { ...element, x: element.x + dx, y: element.y + dy } : element);
  }
  if (added.length) {
    let lockedEnd = 0;
    elements.forEach((element, i) => { if (isLocked(element)) lockedEnd = i + 1; });
    elements.splice(lockedEnd, 0, ...added.filter(isLocked));
    elements.push(...added.filter(element => !isLocked(element)));
  }
  return { ...document, elements, paper };
}

/** A page's picture as a locked image filling `rect`: the backdrop a page copied from a PDF has. */
export function pageBackdrop(rect: Pick<PageRect, 'x' | 'y' | 'width' | 'height'>, image: WhiteboardImportedImage): WhiteboardElement {
  return {
    id: secureId(), type: 'image', x: rect.x, y: rect.y, width: rect.width, height: rect.height,
    dataUrl: image.dataUrl, sourceName: image.name, naturalWidth: image.width, naturalHeight: image.height,
    stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0, locked: true,
  };
}

/** The notebook with one more page after the last, the size and template of that one. */
export function addPage(document: WhiteboardDocument): WhiteboardDocument {
  return document.paper ? { ...document, paper: withAddedPage(document.paper) } : document;
}

/**
 * A blank page at position `at` (0 puts it first, the page count puts it last), the size and template of `like`, by
 * default the page just before that position (or after it, for a new first page). The pages below move down.
 */
export function insertPage(document: WhiteboardDocument, at: number, like?: Pick<PaperPage, 'width' | 'height' | 'template'>): WhiteboardDocument {
  const { paper } = document;
  if (!paper || !Number.isInteger(at) || at < 0 || at > paper.pages.length) return document;
  const model = like ?? paper.pages[at - 1] ?? paper.pages[at];
  const page: PaperPage = { id: nextPageId(paper), width: model.width, height: model.height, ...(model.template ? { template: model.template } : {}) };
  return relayout(document, withPages(paper, spliced(paper.pages, at, page)));
}

/**
 * A copy of page `index` right after it: the same size and template, with a clone of everything on it under new ids.
 * Locked backdrops stay locked; an arrow bound to something on another page loses that binding in its copy.
 */
export function duplicatePage(document: WhiteboardDocument, index: number): WhiteboardDocument {
  const { paper } = document;
  if (!valid(paper, index)) return document;
  const source = paper.pages[index];
  const next = withPages(paper, spliced(paper.pages, index + 1, { ...source, id: nextPageId(paper) }));
  const from = pageRects(paper)[index], to = pageRects(next)[index + 1];
  const owned = document.elements.filter(element => elementPage(paper, element).id === source.id);
  return relayout(document, next, cloneWhiteboardElements(owned, { x: 0, y: to.y - from.y }));
}

/** Page `from` moved to position `to` (its index afterwards); every page's elements go with it. */
export function movePage(document: WhiteboardDocument, from: number, to: number): WhiteboardDocument {
  const { paper } = document;
  if (!valid(paper, from) || !valid(paper, to) || from === to) return document;
  const pages = [...paper.pages];
  pages.splice(to, 0, ...pages.splice(from, 1));
  return relayout(document, withPages(paper, pages));
}

/**
 * The notebook without page `pageId`: the page and everything on it go, and the pages below move up
 * into its place, taking their elements with them. A notebook keeps at least one page.
 */
export function removePage(document: WhiteboardDocument, pageId: string): WhiteboardDocument {
  const { paper } = document;
  if (!paper || paper.pages.length < 2 || !paper.pages.some(page => page.id === pageId)) return document;
  return relayout(document, withPages(paper, paper.pages.filter(page => page.id !== pageId)));
}

/** Page `index` drawn on `template`; nothing moves. */
export function setPageTemplate(document: WhiteboardDocument, index: number, template: PaperTemplate): WhiteboardDocument {
  const { paper } = document;
  if (!valid(paper, index) || templateOf(paper.pages[index]) === template) return document;
  return { ...document, paper: withPages(paper, replaced(paper.pages, index, { ...paper.pages[index], template })) };
}

/** Apply one template to every page while keeping the layout and elements. */
export function setAllPageTemplates(document: WhiteboardDocument, template: PaperTemplate): WhiteboardDocument {
  const { paper } = document;
  if (!paper || paper.pages.every(page => templateOf(page) === template)) return document;
  const pages = paper.pages.map(page => templateOf(page) === template ? page : { ...page, template });
  return { ...document, paper: withPages(paper, pages) };
}

/**
 * Page `index` at `size`. Its elements keep their offset from the page's top-centre; one whose centre would fall off
 * the smaller page is moved just enough to stay on it, and what overhangs is clipped. The pages below move by the
 * change in height.
 */
export function setPageSize(document: WhiteboardDocument, index: number, size: PageSize): WhiteboardDocument {
  const { paper } = document;
  if (!valid(paper, index)) return document;
  const page = paper.pages[index];
  if (page.width === size.width && page.height === size.height) return document;
  return relayout(document, withPages(paper, replaced(paper.pages, index, { ...page, width: size.width, height: size.height })), [], page.id);
}

/**
 * New pages at position `at`, one per picture, each the PDF page's size with its picture laid on it as a locked
 * backdrop (as a notebook made from a PDF has). The pages below move down.
 */
export function insertPdfPages(document: WhiteboardDocument, at: number, sizes: readonly PageSize[], images: readonly WhiteboardImportedImage[]): WhiteboardDocument {
  const { paper } = document;
  if (!paper || !Number.isInteger(at) || at < 0 || at > paper.pages.length || !sizes.length) return document;
  if (sizes.length !== images.length) throw new Error('Each new page needs one picture.');
  const added: PaperPage[] = [];
  for (const size of sizes) {
    const id = nextPageId(withPages(paper, [...paper.pages, ...added]));
    added.push({ id, width: size.width, height: size.height, template: 'blank' });
  }
  const next = withPages(paper, spliced(paper.pages, at, ...added));
  return relayout(document, next, pageRects(next).slice(at, at + added.length).map((rect, i) => pageBackdrop(rect, images[i])));
}
