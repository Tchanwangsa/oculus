import type { Point } from '../model';
import { flip, vectorHeading, type Heading } from './headings';
import { matchLanes, type Lane } from './lanes';
import type { RouteEnd } from './ends';

const EPSILON = 1e-6;
const same = (a: Point, b: Point) => Math.abs(a.x - b.x) < EPSILON && Math.abs(a.y - b.y) < EPSILON;

/** The direction of the first segment of nonzero length, walking `order`. */
function direction(points: readonly Point[], order: readonly number[]): Heading | undefined {
  for (const i of order) {
    const a = points[i], b = points[i + 1];
    if (!same(a, b)) return vectorHeading(b.x - a.x, b.y - a.y);
  }
  return undefined;
}

/**
 * Whether an arrow's current points are still a route for these ends and lanes: it starts and ends on them, every
 * segment is orthogonal, a bound end leaves and arrives along its heading, and every lane has its segment. A route
 * the user shaped by dragging segments stays as it is while this holds, and is routed afresh once an end moves.
 */
export function routeFits(points: readonly Point[], start: RouteEnd, end: RouteEnd, lanes: readonly Lane[]): boolean {
  if (points.length < 2 || !same(points[0], start.point) || !same(points[points.length - 1], end.point)) return false;
  const order = points.slice(1).map((_, i) => i);
  if (order.some(i => Math.abs(points[i].x - points[i + 1].x) >= EPSILON && Math.abs(points[i].y - points[i + 1].y) >= EPSILON)) return false;
  const first = direction(points, order), last = direction(points, [...order].reverse());
  if (start.box && first !== undefined && first !== start.heading) return false;
  if (end.box && last !== undefined && last !== flip(end.heading)) return false;
  return matchLanes(points, lanes).filter(k => k !== undefined).length === lanes.length;
}
