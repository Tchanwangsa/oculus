import { arrowTypeOf, type CanvasElement, type Point } from '../model';
import { catmullRomCubics, flattenCubics, type Cubic } from './curve';

/**
 * The one rendered path of a line, arrow or stroke. Canvas2D and SVG draw its
 * curves; bounds, hit testing, culling and the WASM scene use its polyline, so
 * Rust needs no curve code. Elements are immutable, so the path is cached per element.
 */
export interface LinePath {
  /** The points the path runs through, local to the element's x/y. */
  points: readonly Point[];
  /** Béziers from `points[0]` for a curved arrow of three or more points; empty when the path is its points. */
  curves: readonly Cubic[];
  /** The curves flattened, or the points themselves. */
  polyline: readonly Point[];
  length: number;
}

/** How far the flattened polyline may stray from the curve, in world units. */
const FLATNESS = 0.25;
const cache = new WeakMap<CanvasElement, LinePath>();

export const polylinePoints = (el: CanvasElement): Point[] => el.points?.length ? el.points : [{ x: 0, y: 0 }, { x: el.width, y: el.height }];

export const polylineLength = (points: readonly Point[]) => {
  let length = 0;
  for (let i = 1; i < points.length; i++) length += Math.hypot(points[i].x - points[i - 1].x, points[i].y - points[i - 1].y);
  return length;
};

export function linePath(el: CanvasElement): LinePath {
  let path = cache.get(el);
  if (path) return path;
  const points = polylinePoints(el);
  const curves = el.type === 'arrow' && arrowTypeOf(el) === 'curved' ? catmullRomCubics(points) : [];
  const polyline = curves.length ? flattenCubics(points[0], curves, FLATNESS) : points;
  cache.set(el, path = { points, curves, polyline, length: polylineLength(polyline) });
  return path;
}

/** The visual polyline in element-local coordinates. */
export const arrowPath = (el: CanvasElement): readonly Point[] => linePath(el).polyline;

/** An end's tip and the direction the path arrives at it. */
export interface PathEnd { tip: Point; angle: number }

/** `toward` lists points back along the path from the tip; the first one apart from it sets the direction (0 when none is). */
function endFrom(tip: Point, toward: Iterable<Point>): PathEnd {
  for (const p of toward) if (p.x !== tip.x || p.y !== tip.y) return { tip, angle: Math.atan2(tip.y - p.y, tip.x - p.x) };
  return { tip, angle: 0 };
}

function* backward(points: readonly Point[], end: 'start' | 'end') {
  if (end === 'end') for (let i = points.length - 2; i >= 0; i--) yield points[i];
  else for (let i = 1; i < points.length; i++) yield points[i];
}

/** A polyline's end: direction from its last (or first) segment of nonzero length. */
export const polylineEnd = (points: readonly Point[], end: 'start' | 'end'): PathEnd =>
  endFrom(end === 'end' ? points[points.length - 1] : points[0], backward(points, end));

/** The path's end tangent: a curve's control points, not its last flattened segment. */
export function pathEnd(el: CanvasElement, end: 'start' | 'end'): PathEnd {
  const { points, curves } = linePath(el);
  if (!curves.length) return polylineEnd(points, end);
  if (end === 'start') return endFrom(points[0], [curves[0].c1, curves[0].c2, curves[0].to]);
  const last = curves[curves.length - 1], from = curves.length > 1 ? curves[curves.length - 2].to : points[0];
  return endFrom(last.to, [last.c2, last.c1, from]);
}
