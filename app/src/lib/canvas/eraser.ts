import type { ElementChanges } from './changes';
import { elementBounds } from './geometry';
import type { CanvasElement, Point } from './model';
import { isLocked } from './paper';
import { strokeSamples } from './pens';

/** The distance from `point` to the segment `a`–`b`. */
function distanceToSegment(point: Point, a: Point, b: Point): number {
  const dx = b.x - a.x, dy = b.y - a.y, length = dx * dx + dy * dy;
  const t = length ? Math.max(0, Math.min(1, ((point.x - a.x) * dx + (point.y - a.y) * dy) / length)) : 0;
  return Math.hypot(point.x - a.x - t * dx, point.y - a.y - t * dy);
}

/** A stroke's points close enough together that rubbing out those under the eraser leaves clean ends. */
function dense(points: readonly Point[], step: number): Point[] {
  for (let i = 1; i < points.length; i++) {
    if (Math.hypot(points[i].x - points[i - 1].x, points[i].y - points[i - 1].y) > step * 1.5) return strokeSamples(points, step);
  }
  return [...points];
}

/**
 * Rubs out the parts of freehand strokes within `radius` of the eraser's path from `from` to `to` (world units).
 * A stroke cut in two keeps its ID on the first piece and gets `newId()` for each further one; a stroke with nothing
 * left is removed. Shapes, lines, text, images and locked backdrops are left alone. Only strokes that change are
 * returned.
 */
export function erasePartially(elements: readonly CanvasElement[], from: Point, to: Point, radius: number, newId: () => string): ElementChanges {
  const changes = new Map<string, CanvasElement | null>();
  const left = Math.min(from.x, to.x) - radius, right = Math.max(from.x, to.x) + radius;
  const top = Math.min(from.y, to.y) - radius, bottom = Math.max(from.y, to.y) + radius;
  for (const element of elements) {
    if (element.type !== 'freedraw' || !element.points?.length || isLocked(element)) continue;
    const reach = radius + element.strokeWidth / 2, bounds = elementBounds(element);
    if (bounds.x - reach > right || bounds.x + bounds.width + reach < left || bounds.y - reach > bottom || bounds.y + bounds.height + reach < top) continue;
    const a = { x: from.x - element.x, y: from.y - element.y }, b = { x: to.x - element.x, y: to.y - element.y };
    const under = (point: Point) => distanceToSegment(point, a, b) <= reach;
    const samples = dense(element.points, Math.max(0.5, radius / 2));
    if (!samples.some(under)) continue;
    const runs: Point[][] = [];
    let run: Point[] = [];
    for (const point of samples) {
      if (!under(point)) { run.push(point); continue; }
      if (run.length > 1) runs.push(run);
      run = [];
    }
    if (run.length > 1) runs.push(run);
    changes.set(element.id, null);
    runs.forEach((points, index) => {
      const last = points[points.length - 1], id = index ? newId() : element.id;
      changes.set(id, { ...element, id, width: last.x, height: last.y, points });
    });
  }
  return changes;
}
