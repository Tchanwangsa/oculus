import { arrowheadOf, hasLabel, roundnessOf, type CanvasArrowhead, type CanvasElement, type CanvasViewport, type Point } from './model';
import { arrowPath, linePath, pathEnd, polylineEnd, polylineLength, type PathEnd } from './arrow/path';

export { arrowPath, polylinePoints } from './arrow/path';

export interface Bounds { x: number; y: number; width: number; height: number }

const LINEAR: ReadonlySet<CanvasElement['type']> = new Set(['line', 'arrow', 'freedraw']);

/** A line, arrow or stroke spans its rendered path, so a curve's overshoot past its points counts. */
export function elementBounds(element: CanvasElement): Bounds {
  if (LINEAR.has(element.type)) {
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const p of arrowPath(element)) {
      minX = Math.min(minX, p.x); minY = Math.min(minY, p.y);
      maxX = Math.max(maxX, p.x); maxY = Math.max(maxY, p.y);
    }
    return { x: element.x + minX, y: element.y + minY, width: maxX - minX, height: maxY - minY };
  }
  return { x: Math.min(element.x, element.x + element.width), y: Math.min(element.y, element.y + element.height), width: Math.abs(element.width), height: Math.abs(element.height) };
}

export function selectionBounds(elements: readonly CanvasElement[]): Bounds | null {
  if (!elements.length) return null;
  let x = Infinity, y = Infinity, right = -Infinity, bottom = -Infinity;
  for (const el of elements) {
    const b = elementBounds(el);
    x = Math.min(x, b.x); y = Math.min(y, b.y);
    right = Math.max(right, b.x + b.width); bottom = Math.max(bottom, b.y + b.height);
  }
  return { x, y, width: right - x, height: bottom - y };
}

export function boundsIntersect(a: Bounds, b: Bounds): boolean {
  return a.x <= b.x + b.width && a.x + a.width >= b.x && a.y <= b.y + b.height && a.y + a.height >= b.y;
}

export function boundsContain(outer: Bounds, inner: Bounds): boolean {
  return inner.x >= outer.x && inner.y >= outer.y && inner.x + inner.width <= outer.x + outer.width && inner.y + inner.height <= outer.y + outer.height;
}

export function pointInBounds(p: Point, b: Bounds, padding = 0): boolean {
  return p.x >= b.x - padding && p.y >= b.y - padding && p.x <= b.x + b.width + padding && p.y <= b.y + b.height + padding;
}

/** Viewport x/y are the screen-space translation before the world-space zoom. */
export function screenToWorld(p: Point, v: CanvasViewport): Point { return { x: (p.x - v.x) / v.zoom, y: (p.y - v.y) / v.zoom }; }
export function worldToScreen(p: Point, v: CanvasViewport): Point { return { x: p.x * v.zoom + v.x, y: p.y * v.zoom + v.y }; }
export function zoomAt(v: CanvasViewport, p: Point, zoom: number): CanvasViewport {
  const world = screenToWorld(p, v);
  const next = Math.min(8, Math.max(0.1, zoom));
  return { ...v, zoom: next, x: p.x - world.x * next, y: p.y - world.y * next };
}

export function segmentDistance(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x, dy = b.y - a.y;
  const t = dx || dy ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy))) : 0;
  return Math.hypot(p.x - a.x - t * dx, p.y - a.y - t * dy);
}

export interface ArrowheadShape {
  type: Exclude<CanvasArrowhead, 'none'>;
  tip: Point;
  size: number;
  /** The chevron's wing ends. Hit testing covers tip-to-wing for every type, as Rust does. */
  wings: [Point, Point];
  /** Outline for every type but `dot`; `closed` ones are filled with the stroke colour. */
  points: Point[];
  closed: boolean;
  dot?: { center: Point; radius: number };
}

/**
 * A third of the whole path, so a short last segment of a curve or elbow does not
 * shrink the head; at most 16 so the arrow's 16-unit culling pad in Rust covers every head.
 */
export const arrowheadSize = (pathLength: number) => Math.min(16, pathLength / 3);

/** A head on a polyline, aimed along its end segment; the inspector's glyphs draw with this. */
export function arrowheadShape(points: readonly Point[], end: 'start' | 'end', type: CanvasArrowhead): ArrowheadShape | null {
  if (points.length < 2) return null;
  return arrowheadAt(type, polylineEnd(points, end), arrowheadSize(polylineLength(points)));
}

/** A head at `tip`, pointing along `angle` (the direction the path arrives in). */
export function arrowheadAt(type: CanvasArrowhead, { tip, angle }: PathEnd, size: number): ArrowheadShape | null {
  if (type === 'none') return null;
  const [left, right] = [-Math.PI / 6, Math.PI / 6].map(a => ({ x: tip.x - Math.cos(angle + a) * size, y: tip.y - Math.sin(angle + a) * size }));
  const along = (d: number, side = 0) => ({ x: tip.x - Math.cos(angle) * d - Math.sin(angle) * side, y: tip.y - Math.sin(angle) * d + Math.cos(angle) * side });
  const head = { type, tip, size, wings: [left, right] as [Point, Point], closed: type === 'triangle' || type === 'diamond' };
  if (type === 'dot') return { ...head, points: [], dot: { center: along(size / 2.5), radius: size / 2.5 } };
  if (type === 'bar') return { ...head, points: [along(0, -size / 2), along(0, size / 2)] };
  if (type === 'diamond') return { ...head, points: [tip, along(size / 2, size / 3), along(size), along(size / 2, -size / 3)] };
  return { ...head, points: [left, tip, right] };
}

/** Each head aims along the path's end tangent and is sized from the whole path; the engine record carries the same numbers to Rust. */
export function elementArrowheads(el: CanvasElement): ArrowheadShape[] {
  if (el.type !== 'arrow') return [];
  const size = arrowheadSize(linePath(el).length);
  return (['start', 'end'] as const).flatMap(end => arrowheadAt(arrowheadOf(el, end), pathEnd(el, end), size) ?? []);
}

/** A round rectangle's corner radius, from its size so a resize keeps its look; 0 when sharp. */
export const rectangleRadius = (el: CanvasElement) =>
  roundnessOf(el) === 'round' ? Math.min(8, Math.abs(el.width) / 8, Math.abs(el.height) / 8) : 0;

/** A diamond's top, right, bottom and left vertices, local to its x/y. */
export const diamondVertices = (w: number, h: number): Point[] => [{ x: w / 2, y: 0 }, { x: w, y: h / 2 }, { x: w / 2, y: h }, { x: 0, y: h / 2 }];

/**
 * A round diamond's corners: each is a quadratic curve from `from` to `to` with
 * the vertex as its control, cut a fifth of an edge (at most 12 units) back
 * along both edges. Canvas2D and SVG both draw from this, edges joining the corners.
 */
export function diamondCorners(w: number, h: number): { from: Point; vertex: Point; to: Point }[] {
  const vertices = diamondVertices(w, h), cut = Math.min(12, Math.hypot(w / 2, h / 2) / 5);
  const toward = (a: Point, b: Point) => {
    const length = Math.hypot(b.x - a.x, b.y - a.y) || 1;
    return { x: a.x + (b.x - a.x) * cut / length, y: a.y + (b.y - a.y) * cut / length };
  };
  return vertices.map((vertex, i) => ({ from: toward(vertex, vertices[(i + 3) % 4]), vertex, to: toward(vertex, vertices[(i + 1) % 4]) }));
}

/** Dash lengths grow with the stroke; dotted relies on round caps. Arrowheads always draw solid. */
export function strokeDash(el: CanvasElement): number[] {
  if (el.type === 'freedraw' || el.type === 'text') return [];
  if (el.strokeStyle === 'dashed') return [Math.max(6, 4 * el.strokeWidth), Math.max(4, 3 * el.strokeWidth)];
  if (el.strokeStyle === 'dotted') return [0, 2 * el.strokeWidth + 2];
  return [];
}

/** A label makes a hollow shape selectable across its inside. */
export const elementFilled = (el: CanvasElement) => el.type === 'image' || (el.fill !== 'transparent' && el.fill !== 'none' && el.fill !== '') || hasLabel(el);

export function elementHitTest(el: CanvasElement, world: Point, tolerance = 6): boolean {
  const p = { x: world.x - el.x, y: world.y - el.y };
  const pad = Math.max(0, tolerance) + el.strokeWidth / 2;
  if (LINEAR.has(el.type)) {
    const points = arrowPath(el);
    if (points.length === 1) return Math.hypot(p.x - points[0].x, p.y - points[0].y) <= pad;
    for (let i = 1; i < points.length; i++) if (segmentDistance(p, points[i - 1], points[i]) <= pad) return true;
    return elementArrowheads(el).some(head => head.wings.some(wing => segmentDistance(p, wing, head.tip) <= pad));
  }
  const b = elementBounds(el), filled = elementFilled(el);
  if (!pointInBounds(world, b, pad)) return false;
  if (el.type === 'text') return true;
  if (el.type === 'ellipse') {
    const dx = world.x - b.x - b.width / 2, dy = world.y - b.y - b.height / 2;
    const rx = b.width / 2, ry = b.height / 2;
    const outer = (dx / (rx + pad || 1e-9)) ** 2 + (dy / (ry + pad || 1e-9)) ** 2 <= 1;
    return outer && (filled || rx <= pad || ry <= pad || (dx / (rx - pad)) ** 2 + (dy / (ry - pad)) ** 2 >= 1);
  }
  if (el.type === 'diamond') {
    const vertices = [{ x: b.x + b.width / 2, y: b.y }, { x: b.x + b.width, y: b.y + b.height / 2 }, { x: b.x + b.width / 2, y: b.y + b.height }, { x: b.x, y: b.y + b.height / 2 }];
    if (filled && b.width && b.height && Math.abs((world.x - b.x - b.width / 2) / (b.width / 2)) + Math.abs((world.y - b.y - b.height / 2) / (b.height / 2)) <= 1) return true;
    return vertices.some((a, i) => segmentDistance(world, a, vertices[(i + 1) % 4]) <= pad);
  }
  return filled || world.x <= b.x + pad || world.x >= b.x + b.width - pad || world.y <= b.y + pad || world.y >= b.y + b.height - pad;
}

/** Iterative Ramer–Douglas–Peucker keeps long pointer strokes off the call stack. */
export function simplifyPolyline(points: readonly Point[], tolerance = 1): Point[] {
  if (points.length <= 2) return points.map(p => ({ ...p }));
  const keep = new Uint8Array(points.length); keep[0] = keep[points.length - 1] = 1;
  const stack: [number, number][] = [[0, points.length - 1]];
  while (stack.length) {
    const [start, end] = stack.pop()!;
    let far = Math.max(0, tolerance), index = -1;
    for (let i = start + 1; i < end; i++) {
      const d = segmentDistance(points[i], points[start], points[end]);
      if (d > far) { far = d; index = i; }
    }
    if (index >= 0) { keep[index] = 1; stack.push([start, index], [index, end]); }
  }
  return points.filter((_, i) => keep[i]).map(p => ({ ...p }));
}
