import type { CanvasElement, Point } from './model';
import { elementBounds } from './geometry';

/**
 * Outline geometry of the shapes an arrow can bind to. Text binds to its bounds
 * rectangle; a rectangle's rounded corners are ignored, which the arrow's gap
 * hides. Every function returns a finite point for zero-size shapes.
 */
export const BINDABLE_TYPES: ReadonlySet<CanvasElement['type']> = new Set(['rectangle', 'ellipse', 'diamond', 'text']);
export const isBindable = (el: CanvasElement) => BINDABLE_TYPES.has(el.type);

export function outlineCenter(el: CanvasElement): Point {
  const b = elementBounds(el);
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
}

/** Top, right, bottom and left: these lie on every bindable kind's outline. */
export function outlineMidpoints(el: CanvasElement): Point[] {
  const b = elementBounds(el), cx = b.x + b.width / 2, cy = b.y + b.height / 2;
  return [{ x: cx, y: b.y }, { x: b.x + b.width, y: cy }, { x: cx, y: b.y + b.height }, { x: b.x, y: cy }];
}

const ratio = (d: number, half: number) => half > 0 ? Math.abs(d) / half : d === 0 ? 0 : Infinity;
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** How far an offset from the centre reaches toward the outline: 1 on it, under 1 inside. Each kind is convex and scales about its centre. */
export function outlineGauge(el: CanvasElement, offset: Point): number {
  const b = elementBounds(el), rx = ratio(offset.x, b.width / 2), ry = ratio(offset.y, b.height / 2);
  if (el.type === 'ellipse') return Math.hypot(rx, ry);
  if (el.type === 'diamond') return rx + ry;
  return Math.max(rx, ry);
}

/** `slack` counts points this far past the outline (as a fraction of the shape) as inside, so a point on it stays inside despite rounding. */
export function pointInsideOutline(el: CanvasElement, p: Point, slack = 0): boolean {
  const c = outlineCenter(el);
  return outlineGauge(el, { x: p.x - c.x, y: p.y - c.y }) <= 1 + slack;
}

/** Where the ray from the centre through `toward` meets the outline; the centre when that ray has no direction or never meets it. */
export function outlineAlong(el: CanvasElement, toward: Point): Point {
  const c = outlineCenter(el), d = { x: toward.x - c.x, y: toward.y - c.y };
  const gauge = outlineGauge(el, d);
  if (!(gauge > 0) || !Number.isFinite(gauge)) return c;
  return { x: c.x + d.x / gauge, y: c.y + d.y / gauge };
}

function closestOnSegment(p: Point, a: Point, b: Point): Point {
  const dx = b.x - a.x, dy = b.y - a.y, length = dx * dx + dy * dy;
  const t = length ? clamp(((p.x - a.x) * dx + (p.y - a.y) * dy) / length, 0, 1) : 0;
  return { x: a.x + t * dx, y: a.y + t * dy };
}

/** Nearest point on an ellipse by a few trig-free evolute iterations (0xfaded's method), solved in the first quadrant and mirrored. */
function nearestOnEllipse(c: Point, a: number, b: number, p: Point): Point {
  if (a <= 0 || b <= 0) return { x: c.x + clamp(p.x - c.x, -a, a), y: c.y + clamp(p.y - c.y, -b, b) };
  const px = Math.abs(p.x - c.x), py = Math.abs(p.y - c.y);
  let tx = Math.SQRT1_2, ty = Math.SQRT1_2;
  for (let i = 0; i < 4; i++) {
    const ex = (a * a - b * b) * tx ** 3 / a, ey = (b * b - a * a) * ty ** 3 / b;
    const rx = a * tx - ex, ry = b * ty - ey, qx = px - ex, qy = py - ey;
    const q = Math.hypot(qx, qy), scale = q > 0 ? Math.hypot(rx, ry) / q : 0;
    tx = clamp((qx * scale + ex) / a, 0, 1); ty = clamp((qy * scale + ey) / b, 0, 1);
    const t = Math.hypot(tx, ty) || 1; tx /= t; ty /= t;
  }
  return { x: c.x + (p.x < c.x ? -a : a) * tx, y: c.y + (p.y < c.y ? -b : b) * ty };
}

export function nearestOutlinePoint(el: CanvasElement, p: Point): Point {
  const b = elementBounds(el), c = outlineCenter(el);
  if (el.type === 'ellipse') return nearestOnEllipse(c, b.width / 2, b.height / 2, p);
  if (el.type === 'diamond') {
    const [top, right, bottom, left] = outlineMidpoints(el);
    let best = top, far = Infinity;
    for (const [a, z] of [[top, right], [right, bottom], [bottom, left], [left, top]]) {
      const q = closestOnSegment(p, a, z), d = Math.hypot(q.x - p.x, q.y - p.y);
      if (d < far) { far = d; best = q; }
    }
    return best;
  }
  const right = b.x + b.width, bottom = b.y + b.height;
  const x = clamp(p.x, b.x, right), y = clamp(p.y, b.y, bottom);
  if (x !== p.x || y !== p.y) return { x, y };
  const gaps = [p.y - b.y, right - p.x, bottom - p.y, p.x - b.x], nearest = gaps.indexOf(Math.min(...gaps));
  return [{ x, y: b.y }, { x: right, y }, { x, y: bottom }, { x: b.x, y }][nearest];
}

/** An outline point stored relative to the bounds, so it stays on the outline as the shape moves or resizes. */
export function outlineAnchor(el: CanvasElement, p: Point): Point {
  const b = elementBounds(el);
  return { x: b.width > 0 ? clamp((p.x - b.x) / b.width, 0, 1) : 0.5, y: b.height > 0 ? clamp((p.y - b.y) / b.height, 0, 1) : 0.5 };
}

export function anchorPoint(el: CanvasElement, anchor: Point): Point {
  const b = elementBounds(el);
  return outlineAlong(el, { x: b.x + anchor.x * b.width, y: b.y + anchor.y * b.height });
}
