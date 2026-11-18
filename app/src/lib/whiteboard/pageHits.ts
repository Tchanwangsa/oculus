/**
 * A notebook clips each element to its own page, so the pointer must not find the hidden part hanging off it. These
 * answer hit tests with only what is drawn: the elements a point can reach, and the bounds a box selection sees.
 */
import { elementBounds, type Bounds } from './geometry';
import type { WhiteboardElement, Point } from './model';
import { pageContaining, type Paper } from './paper';
import { elementPage } from './renderPaper';

const NONE: readonly WhiteboardElement[] = [];
const byPage = new WeakMap<readonly WhiteboardElement[], { paper: Paper; pages: Map<string, readonly WhiteboardElement[]> }>();

/**
 * The elements of `elements` whose own page holds `point` (none on the desk), in order. Cached per array, paper and
 * page, so the hit-test engine keeps reusing its scene while the document is unchanged.
 */
export function elementsAtPage(paper: Paper, elements: readonly WhiteboardElement[], point: Point): readonly WhiteboardElement[] {
  const page = pageContaining(paper, point);
  if (!page) return NONE;
  let entry = byPage.get(elements);
  if (!entry || entry.paper !== paper) byPage.set(elements, entry = { paper, pages: new Map() });
  let found = entry.pages.get(page.id);
  if (!found) entry.pages.set(page.id, found = elements.filter(element => elementPage(paper, element).id === page.id));
  return found;
}

/** The part of `element`'s bounds its page shows, or null when none of it is on the page. */
export function shownBounds(paper: Paper, element: WhiteboardElement): Bounds | null {
  const b = elementBounds(element), page = elementPage(paper, element);
  const left = Math.max(b.x, page.x), right = Math.min(b.x + b.width, page.x + page.width);
  const top = Math.max(b.y, page.y), bottom = Math.min(b.y + b.height, page.y + page.height);
  return left <= right && top <= bottom ? { x: left, y: top, width: right - left, height: bottom - top } : null;
}
