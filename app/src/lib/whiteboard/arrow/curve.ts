import type { Point } from '../model';

/** A cubic Bézier that starts where the previous one (or the path) ends. */
export interface Cubic { c1: Point; c2: Point; to: Point }

/**
 * A uniform Catmull–Rom spline through `points` as cubic Béziers, each end
 * repeated so the curve starts and ends on them: roughjs's `curve`, which
 * Excalidraw draws round arrows with. Uniform keeps it affine, so scaling the
 * points scales the curve. Under three points there is no curve.
 */
export function catmullRomCubics(points: readonly Point[]): Cubic[] {
  if (points.length < 3) return [];
  const at = (i: number) => points[Math.max(0, Math.min(points.length - 1, i))];
  const cubics: Cubic[] = [];
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = at(i - 1), p1 = at(i), p2 = at(i + 1), p3 = at(i + 2);
    cubics.push({
      c1: { x: p1.x + (p2.x - p0.x) / 6, y: p1.y + (p2.y - p0.y) / 6 },
      c2: { x: p2.x - (p3.x - p1.x) / 6, y: p2.y - (p3.y - p1.y) / 6 },
      to: { x: p2.x, y: p2.y },
    });
  }
  return cubics;
}

export function cubicAt(from: Point, { c1, c2, to }: Cubic, t: number): Point {
  const u = 1 - t, a = u * u * u, b = 3 * u * u * t, c = 3 * u * t * t, d = t * t * t;
  return { x: a * from.x + b * c1.x + c * c2.x + d * to.x, y: a * from.y + b * c1.y + c * c2.y + d * to.y };
}

/**
 * The curves as a polyline no further than `tolerance` from them, by Wang's
 * bound on each cubic's second differences. Every curve's end is a vertex, so
 * the polyline passes exactly through the points the curve was built from.
 */
export function flattenCubics(start: Point, cubics: readonly Cubic[], tolerance: number): Point[] {
  const out = [{ x: start.x, y: start.y }];
  let from = start;
  for (const cubic of cubics) {
    const { c1, c2, to } = cubic;
    const reach = Math.max(Math.hypot(from.x - 2 * c1.x + c2.x, from.y - 2 * c1.y + c2.y), Math.hypot(c1.x - 2 * c2.x + to.x, c1.y - 2 * c2.y + to.y));
    const steps = Math.min(64, Math.max(1, Math.ceil(Math.sqrt((0.75 * reach) / tolerance))));
    for (let i = 1; i < steps; i++) out.push(cubicAt(from, cubic, i / steps));
    out.push({ x: to.x, y: to.y });
    from = to;
  }
  return out;
}
