import { arrowTypeOf, type CanvasElement, type Point } from '../model';
import { cubicAt } from './curve';
import { linePath } from './path';
import { insertPoint } from './points';

/**
 * The handles a single selected line or arrow shows, in world coordinates. A
 * `point` handle moves point `index`; a `midpoint` sits halfway along segment
 * `segment` (on the curve, for a curved arrow) and dragging it inserts a point there;
 * a `segment` handle sits halfway along an elbow arrow's segment and drags that segment (`elbow/segments.ts`).
 */
export type PointHandle =
  | { kind: 'point'; index: number; at: Point }
  | { kind: 'midpoint'; segment: number; at: Point }
  | { kind: 'segment'; segment: number; at: Point };

/** A segment shorter than this on screen shows no midpoint, which would crowd its ends. */
const MIDPOINT_MIN_SCREEN = 28;
const HANDLE_RANGE = 10;

/**
 * Ends always. A sharp or curved arrow also has every interior point and a midpoint per segment; an elbow arrow a
 * segment handle per segment. Lines edit only their ends.
 */
export function pointHandles(el: CanvasElement, zoom: number): PointHandle[] {
  const { points, curves } = linePath(el);
  const world = (p: Point) => ({ x: el.x + p.x, y: el.y + p.y });
  const last = points.length - 1, elbow = arrowTypeOf(el) === 'elbow';
  const ends: PointHandle[] = [0, last].map(index => ({ kind: 'point', index, at: world(points[index]) }));
  if (el.type !== 'arrow') return ends;
  const handles: PointHandle[] = elbow ? ends : points.map((p, index) => ({ kind: 'point', index, at: world(p) }));
  for (let i = 0; i < last; i++) {
    const a = points[i], b = points[i + 1];
    if (Math.hypot(b.x - a.x, b.y - a.y) * zoom < MIDPOINT_MIN_SCREEN) continue;
    const mid = curves.length ? cubicAt(a, curves[i], 0.5) : { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    handles.push({ kind: elbow ? 'segment' : 'midpoint', segment: i, at: world(mid) });
  }
  return handles;
}

/** The handle under the pointer: the nearest point within range, else the nearest midpoint or segment handle, so points win where they overlap. */
export function pointHandleAt(el: CanvasElement, world: Point, zoom: number): PointHandle | null {
  let best: PointHandle | null = null, far = HANDLE_RANGE / zoom;
  const handles = pointHandles(el, zoom);
  for (const point of [true, false]) {
    for (const handle of handles) {
      const d = Math.hypot(handle.at.x - world.x, handle.at.y - world.y);
      if ((handle.kind === 'point') === point && d <= far) { far = d; best = handle; }
    }
    if (best) return best;
  }
  return null;
}

/** What a point or midpoint drag edits: the element (with a midpoint's new point already in it) and the index of the point that follows the pointer. */
export function pointDrag(el: CanvasElement, handle: Exclude<PointHandle, { kind: 'segment' }>): { base: CanvasElement; index: number } {
  if (handle.kind === 'point') return { base: el, index: handle.index };
  return { base: insertPoint(el, handle.segment, handle.at), index: handle.segment + 1 };
}
