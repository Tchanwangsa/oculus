import type { CanvasElement } from './model';
import { boundsIntersect, elementBounds } from './geometry';

/** Paint order is array order, last on top. */
export type LayerMove = 'front' | 'forward' | 'backward' | 'back';

const unchanged = (a: readonly CanvasElement[], b: readonly CanvasElement[]) => a.every((element, i) => element === b[i]);

/**
 * Moves the selected elements in paint order, keeping their order among themselves; returns
 * the input array when nothing moves. Forward and backward step each element past the nearest
 * unselected element that overlaps it, or one place when none does before the next selected
 * element or the end of the stack.
 */
export function reorderElements(elements: CanvasElement[], ids: ReadonlySet<string>, move: LayerMove): CanvasElement[] {
  if (!ids.size) return elements;
  let next: CanvasElement[];
  if (move === 'front' || move === 'back') {
    const chosen = elements.filter(element => ids.has(element.id)), rest = elements.filter(element => !ids.has(element.id));
    next = move === 'front' ? [...rest, ...chosen] : [...chosen, ...rest];
  } else {
    next = elements.slice();
    const up = move === 'forward', step = up ? 1 : -1;
    // The element nearest the destination moves first, so the rest stop beneath (or above) it.
    for (let i = up ? next.length - 1 : 0; i >= 0 && i < next.length; i -= step) {
      const element = next[i];
      if (!ids.has(element.id)) continue;
      const bounds = elementBounds(element);
      let target = -1;
      for (let j = i + step; j >= 0 && j < next.length && !ids.has(next[j].id); j += step) {
        if (boundsIntersect(bounds, elementBounds(next[j]))) { target = j; break; }
      }
      if (target < 0) {
        const neighbour = next[i + step];
        if (!neighbour || ids.has(neighbour.id)) continue;
        target = i + step;
      }
      next.splice(i, 1);
      next.splice(target, 0, element);
    }
  }
  return unchanged(elements, next) ? elements : next;
}

/** Whether `move` would change anything: the selection is not already the top (or bottom) of the stack. */
export function canReorder(elements: readonly CanvasElement[], ids: ReadonlySet<string>, move: LayerMove): boolean {
  if (!ids.size) return false;
  const up = move === 'front' || move === 'forward';
  let edge = true;
  for (let k = 0; k < elements.length; k++) {
    const selected = ids.has(elements[up ? elements.length - 1 - k : k].id);
    if (selected && !edge) return true;
    if (!selected) edge = false;
  }
  return false;
}
