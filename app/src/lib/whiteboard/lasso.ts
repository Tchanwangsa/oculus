import { elementBounds } from './geometry';
import type { WhiteboardElement, Point } from './model';
import { isLocked } from './paper';

/** Whether `point` is inside the closed `polygon` (even-odd, so a loop that crosses itself has holes). */
export function pointInPolygon(point: Point, polygon: readonly Point[]): boolean {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    const a = polygon[i], b = polygon[j];
    if ((a.y > point.y) !== (b.y > point.y) && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x) inside = !inside;
  }
  return inside;
}

/** The points that stand for an element: a stroke or line's own points, otherwise its centre and corners. */
function probes(element: WhiteboardElement): Point[] {
  if (element.points?.length && element.type !== 'text') return element.points.map(point => ({ x: element.x + point.x, y: element.y + point.y }));
  const { x, y, width, height } = elementBounds(element);
  return [{ x: x + width / 2, y: y + height / 2 }, { x, y }, { x: x + width, y }, { x, y: y + height }, { x: x + width, y: y + height }];
}

/**
 * The elements a lasso loop takes: those with more than half of their points inside it, so a loop drawn roughly
 * round some writing takes it without taking a neighbour it only clips. Locked backdrops are never taken.
 */
export function lassoSelection(elements: readonly WhiteboardElement[], loop: readonly Point[]): string[] {
  if (loop.length < 3) return [];
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
  for (const { x, y } of loop) { left = Math.min(left, x); right = Math.max(right, x); top = Math.min(top, y); bottom = Math.max(bottom, y); }
  const ids: string[] = [];
  for (const element of elements) {
    if (isLocked(element)) continue;
    const bounds = elementBounds(element);
    if (bounds.x > right || bounds.x + bounds.width < left || bounds.y > bottom || bounds.y + bounds.height < top) continue;
    const points = probes(element);
    if (points.filter(point => pointInPolygon(point, loop)).length * 2 > points.length) ids.push(element.id);
  }
  return ids;
}
