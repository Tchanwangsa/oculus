import { boundsIntersect, elementBounds } from './geometry';
import type { CanvasDocument, CanvasElement } from './model';
import { pageRects } from './paper';

/** An element's text, kept per element object so an untouched one is never serialised twice. An embedded picture counts by its size and tail. */
const elementKeys = new WeakMap<CanvasElement, string>();
function elementKey(element: CanvasElement): string {
  let key = elementKeys.get(element);
  if (key === undefined) {
    const { dataUrl, ...rest } = element;
    key = JSON.stringify(rest) + (dataUrl ? `|${dataUrl.length}:${dataUrl.slice(-48)}` : '');
    elementKeys.set(element, key);
  }
  return key;
}

/** What each page's picture depends on, by page id: its number, size and template, and every element touching it. */
export function pageFingerprints(document: CanvasDocument): Map<string, string> {
  const { paper } = document;
  if (!paper) return new Map();
  return new Map(pageRects(paper).map(rect => {
    const touching = document.elements.filter(element => boundsIntersect(elementBounds(element), rect));
    return [rect.id, `${rect.index} ${rect.width}x${rect.height} ${rect.template}\n${touching.map(elementKey).join('\n')}`];
  }));
}
