import type { CanvasDocument } from './model';
import { gapOf, pageOfElement, pageRects, withAddedPage, withTemplate, type PaperTemplate } from './paper';

/** The notebook with one more page after the last, the size and template of that one. */
export function addPage(document: CanvasDocument): CanvasDocument {
  return document.paper ? { ...document, paper: withAddedPage(document.paper) } : document;
}

/** The notebook with `template` on page `pageId`, or on every page without one. Nothing drawn on a page moves. */
export function setPageTemplate(document: CanvasDocument, template: PaperTemplate, pageId?: string): CanvasDocument {
  const paper = document.paper && withTemplate(document.paper, template, pageId);
  return paper && paper !== document.paper ? { ...document, paper } : document;
}

/**
 * The notebook without page `pageId`: the page and everything on it go, and the pages below move up
 * into its place, taking their elements with them. A notebook keeps at least one page.
 */
export function removePage(document: CanvasDocument, pageId: string): CanvasDocument {
  const { paper } = document;
  const gone = paper && pageRects(paper).find(rect => rect.id === pageId);
  if (!paper || !gone || paper.pages.length < 2) return document;
  const shift = gone.height + gapOf(paper);
  const elements = [];
  for (const element of document.elements) {
    const owner = pageOfElement(paper, element).index;
    if (owner === gone.index) continue;
    elements.push(owner > gone.index ? { ...element, y: element.y - shift } : element);
  }
  return { ...document, elements, paper: { ...paper, pages: paper.pages.filter(page => page.id !== pageId) } };
}
