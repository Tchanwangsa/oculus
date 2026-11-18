/**
 * What a selection box selects. The release and the live preview while dragging both ask `boxSelection`, so the
 * highlight always shows what letting go will select.
 */
import { boundsContain, elementBounds, type Bounds } from './geometry';
import type { WhiteboardElement } from './model';
import { isLocked, type Paper } from './paper';
import { shownBounds } from './pageHits';

const cache = new WeakMap<WhiteboardElement, { paper: Paper | undefined; bounds: Bounds | null }>();

/** The bounds a box must enclose, cached per element: in a notebook only the part its page shows (`shownBounds`). */
export function boxBounds(element: WhiteboardElement, paper: Paper | undefined): Bounds | null {
  const cached = cache.get(element);
  if (cached && cached.paper === paper) return cached.bounds;
  const bounds = paper ? shownBounds(paper, element) : elementBounds(element);
  cache.set(element, { paper, bounds });
  return bounds;
}

/**
 * The selection a box leaves: with `add` (Shift) on top of `current`, otherwise alone. A press that never moved on an
 * element (`toggle`) toggles just that element; otherwise every unlocked element whose bounds fit entirely inside joins.
 */
export function boxSelection(elements: readonly WhiteboardElement[], box: Bounds, paper: Paper | undefined, current: ReadonlySet<string>, { add, toggle }: { add: boolean; toggle?: string }): Set<string> {
  const ids = add ? new Set(current) : new Set<string>();
  if (toggle) {
    if (ids.has(toggle)) ids.delete(toggle); else ids.add(toggle);
    return ids;
  }
  for (const element of elements) {
    if (isLocked(element)) continue;
    const bounds = boxBounds(element, paper);
    if (bounds && boundsContain(box, bounds)) ids.add(element.id);
  }
  return ids;
}
