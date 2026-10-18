import type { Bounds } from '../geometry';
import type { Point } from '../model';
import { DOWN, RIGHT, UP, type Heading } from './headings';
import type { Lane } from './lanes';

/** An axis-aligned box by its edges, which is how the grid reads it. */
export interface Box { left: number; top: number; right: number; bottom: number }

/** How far a route keeps clear of a bound target, where it is not between the two targets. */
export const PADDING = 40;

export const boxOf = (b: Bounds): Box => ({ left: b.x, top: b.y, right: b.x + b.width, bottom: b.y + b.height });
export const pointBox = (p: Point): Box => ({ left: p.x, top: p.y, right: p.x, bottom: p.y });

/** The gap from `a` to `b` along one axis when `b` lies wholly past `a` on it, else null. */
const gapX = (a: Box, b: Box) => b.left >= a.right ? b.left - a.right : a.left >= b.right ? a.left - b.right : null;
const gapY = (a: Box, b: Box) => b.top >= a.bottom ? b.top - a.bottom : a.top >= b.bottom ? a.top - b.bottom : null;

/**
 * `box` padded by PADDING, except that the side facing `other` across a gap reaches halfway across it, so the two
 * padded boxes meet on the midline (a route between them runs there) and never overlap; diagonal boxes meet at a
 * corner. Padding each box against the other is symmetric.
 */
export function padBox(box: Box, other: Box): Box {
  const gx = gapX(box, other), gy = gapY(box, other);
  const pad = (gap: number | null, facing: boolean) => gap !== null && facing ? gap / 2 : PADDING;
  return {
    left: box.left - pad(gx, other.right <= box.left), right: box.right + pad(gx, other.left >= box.right),
    top: box.top - pad(gy, other.bottom <= box.top), bottom: box.bottom + pad(gy, other.top >= box.bottom),
  };
}

/**
 * A padded box drawn in to any fixed segment's line that runs between the body and the padding, so the route can
 * take that line past the body; a line through the body leaves the box as it is.
 */
export function clipToLanes(padded: Box, body: Box, lanes: readonly Lane[]): Box {
  const out = { ...padded };
  for (const { horizontal, at } of lanes) {
    if (horizontal) {
      if (at <= body.top && at > out.top) out.top = at;
      if (at >= body.bottom && at < out.bottom) out.bottom = at;
    } else {
      if (at <= body.left && at > out.left) out.left = at;
      if (at >= body.right && at < out.right) out.right = at;
    }
  }
  return out;
}

/** Where an end's stub leaves its padded box: the end point carried along its heading to the box's side, or the point itself when it is already past it. */
export function dongle(point: Point, heading: Heading, padded?: Box): Point {
  if (!padded) return point;
  if (heading === UP) return { x: point.x, y: Math.min(point.y, padded.top) };
  if (heading === RIGHT) return { x: Math.max(point.x, padded.right), y: point.y };
  if (heading === DOWN) return { x: point.x, y: Math.max(point.y, padded.bottom) };
  return { x: Math.min(point.x, padded.left), y: point.y };
}

/**
 * The sparse grid: every edge of the two bodies (padded boxes, or a free end's point), the dongles, and the line
 * midway between the bodies on an axis they are apart on. Fixed segments add their lines and a frame PADDING outside
 * everything, so a route can always reach a line and run some way along it.
 */
export function gridLines(a: Box, b: Box, dongles: readonly Point[], lanes: readonly Lane[] = []): { xs: number[]; ys: number[] } {
  const xs = new Set([a.left, a.right, b.left, b.right, ...dongles.map(p => p.x)]);
  const ys = new Set([a.top, a.bottom, b.top, b.bottom, ...dongles.map(p => p.y)]);
  if (a.right < b.left) xs.add((a.right + b.left) / 2);
  if (b.right < a.left) xs.add((b.right + a.left) / 2);
  if (a.bottom < b.top) ys.add((a.bottom + b.top) / 2);
  if (b.bottom < a.top) ys.add((b.bottom + a.top) / 2);
  for (const lane of lanes) (lane.horizontal ? ys : xs).add(lane.at);
  if (lanes.length) {
    for (const values of [xs, ys]) {
      const all = [...values];
      values.add(Math.min(...all) - PADDING).add(Math.max(...all) + PADDING);
    }
  }
  const sorted = (values: Set<number>) => [...values].sort((p, q) => p - q);
  return { xs: sorted(xs), ys: sorted(ys) };
}

const EPSILON = 1e-6;
/** Whether an axis-aligned segment passes through the open inside of a box; running along its edge does not. */
export function segmentEntersBox(a: Point, b: Point, box: Box): boolean {
  const left = Math.min(a.x, b.x), right = Math.max(a.x, b.x), top = Math.min(a.y, b.y), bottom = Math.max(a.y, b.y);
  return right > box.left + EPSILON && left < box.right - EPSILON && bottom > box.top + EPSILON && top < box.bottom - EPSILON;
}
