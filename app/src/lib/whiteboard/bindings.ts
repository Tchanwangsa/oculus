import { arrowTypeOf, type CanvasBinding, type CanvasElement, type Point } from './model';
import { elementBounds, pointInBounds, polylinePoints } from './geometry';
import { anchorPoint, isBindable, nearestOutlinePoint, outlineAlong, outlineAnchor, outlineCenter, outlineMidpoints, pointInsideOutline } from './outline';
import type { ElementChanges } from './changes';
import { routeElbow } from './elbow';
import { withoutFixedSegments } from './elbow/lanes';

/**
 * Arrow ends bound to shapes: where a pointer binds, where a bound end sits,
 * and the derived edits that keep bound arrows attached as shapes change.
 * Distances in screen pixels are divided by the zoom so they feel the same at any scale.
 */
export type ArrowEnd = 'start' | 'end';
export interface BindingSnap { target: CanvasElement; binding: CanvasBinding; point: Point }
type Live = ReadonlyMap<string, CanvasElement | null>;

const BIND_RANGE = 16, MIDPOINT_RANGE = 12, CENTER_RANGE = 10;
export const bindingKey = (end: ArrowEnd) => end === 'start' ? 'startBinding' : 'endBinding';

export function withoutBinding(el: CanvasElement, end: ArrowEnd): CanvasElement {
  const key = bindingKey(end);
  if (el[key] === undefined) return el;
  const { [key]: _removed, ...rest } = el;
  return rest as CanvasElement;
}

/**
 * The topmost bindable shape near `point` (live geometry where `live` overrides it) and how an end there binds:
 * its central zone binds to the centre, elsewhere to the nearest outline point, pulled onto an edge midpoint when close.
 * `avoidCenter` is a shape the other end already binds to by its centre, which this end may only bind to by an edge.
 */
export function findBindingSnap(elements: readonly CanvasElement[], point: Point, zoom: number, options: { live?: Live; avoidCenter?: string } = {}): BindingSnap | null {
  for (let i = elements.length - 1; i >= 0; i--) {
    const el = options.live?.has(elements[i].id) ? options.live.get(elements[i].id) : elements[i];
    if (!el || !isBindable(el)) continue;
    const b = elementBounds(el);
    if (!pointInBounds(point, b, BIND_RANGE / zoom)) continue;
    const c = outlineCenter(el);
    if (el.id !== options.avoidCenter && Math.hypot(point.x - c.x, point.y - c.y) <= Math.max(CENTER_RANGE / zoom, 0.2 * Math.min(b.width, b.height))) {
      return { target: el, binding: { elementId: el.id, mode: 'center' }, point: c };
    }
    let edge = nearestOutlinePoint(el, point), near = MIDPOINT_RANGE / zoom;
    for (const midpoint of outlineMidpoints(el)) {
      const d = Math.hypot(midpoint.x - point.x, midpoint.y - point.y);
      if (d <= near) { near = d; edge = midpoint; }
    }
    return { target: el, binding: { elementId: el.id, mode: 'edge', anchor: outlineAnchor(el, edge) }, point: edge };
  }
  return null;
}

export const sameSnap = (a: BindingSnap | null, b: BindingSnap | null) =>
  a === b || (!!a && !!b && a.target === b.target && a.binding.mode === b.binding.mode && a.point.x === b.point.x && a.point.y === b.point.y);

/**
 * Binds one end of an arrow at the pointer, or unbinds it when nothing is there or `free` (⌘/Ctrl) is held.
 * An elbow arrow is re-routed here, bound or not, since `followBindings` only re-places bound arrows; binding an end
 * to a different shape drops its fixed segments, which were placed for the old one.
 */
export function bindArrowEnd(arrow: CanvasElement, end: ArrowEnd, point: Point, elements: readonly CanvasElement[], zoom: number, options: { live?: Live; free?: boolean } = {}) {
  const other = arrow[bindingKey(end === 'start' ? 'end' : 'start')];
  const avoidCenter = other?.mode === 'center' ? other.elementId : undefined;
  let snap = options.free ? null : findBindingSnap(elements, point, zoom, { live: options.live, avoidCenter });
  // An elbow end leaves by a side: well inside a shape, the side facing the other end reads better than the nearest edge.
  if (snap?.binding.mode === 'edge' && arrowTypeOf(arrow) === 'elbow' && snap.target.id !== avoidCenter && pointInsideOutline(snap.target, point)) {
    const edge = nearestOutlinePoint(snap.target, point);
    if (Math.hypot(point.x - edge.x, point.y - edge.y) > BIND_RANGE / zoom) snap = { ...snap, binding: { elementId: snap.target.id, mode: 'center' }, point: outlineCenter(snap.target) };
  }
  const rebound = snap && snap.target.id !== arrow[bindingKey(end)]?.elementId;
  const bound = snap ? { ...(rebound ? withoutFixedSegments(arrow) : arrow), [bindingKey(end)]: snap.binding } : withoutBinding(arrow, end);
  if (arrowTypeOf(bound) !== 'elbow') return { arrow: bound, snap };
  return { arrow: routeElbow(bound, sceneLookup(elements, options.live)), snap };
}

const same = (a: Point, b: Point) => Math.abs(a.x - b.x) < 1e-9 && Math.abs(a.y - b.y) < 1e-9;

/** A bound end before clipping: an edge anchor's outline point, or the centre. */
const targetPoint = (binding: CanvasBinding, target: CanvasElement) => binding.mode === 'edge' ? anchorPoint(target, binding.anchor) : outlineCenter(target);

/** Centre mode crosses the outline on the way to `other`; when `other` is inside the shape (or on it) there is no crossing and the end stays at the centre. */
function placeEnd(binding: CanvasBinding, target: CanvasElement, raw: Point, other: Point, gap: number): Point {
  if (binding.mode === 'center') {
    if (pointInsideOutline(target, other, 1e-6)) return raw;
    raw = outlineAlong(target, other);
  }
  const d = Math.hypot(other.x - raw.x, other.y - raw.y);
  if (!(d > 0)) return raw;
  const t = Math.min(gap, d / 2) / d;
  return { x: raw.x + (other.x - raw.x) * t, y: raw.y + (other.y - raw.y) * t };
}

/**
 * Places an arrow's bound ends on its targets, each pulled back toward the other end by 4 + strokeWidth/2.
 * Both raw targets are found first so two bound ends aim at each other. An elbow arrow is routed afresh instead
 * (`routeElbow`). Returns the arrow itself when nothing moves.
 */
export function resolveArrow(arrow: CanvasElement, lookup: (id: string) => CanvasElement | null | undefined): CanvasElement {
  if (arrow.type !== 'arrow') return arrow;
  const target = (binding?: CanvasBinding) => {
    const el = binding ? lookup(binding.elementId) : undefined;
    return el && isBindable(el) ? el : undefined;
  };
  const startTarget = target(arrow.startBinding), endTarget = target(arrow.endBinding);
  if (!startTarget && !endTarget) return arrow;
  if (arrowTypeOf(arrow) === 'elbow') return routeElbow(arrow, lookup);
  const world = polylinePoints(arrow).map(p => ({ x: arrow.x + p.x, y: arrow.y + p.y }));
  const last = world.length - 1, gap = 4 + arrow.strokeWidth / 2;
  const rawStart = startTarget ? targetPoint(arrow.startBinding!, startTarget) : world[0];
  const rawEnd = endTarget ? targetPoint(arrow.endBinding!, endTarget) : world[last];
  const start = startTarget ? placeEnd(arrow.startBinding!, startTarget, rawStart, last > 1 ? world[1] : rawEnd, gap) : world[0];
  const end = endTarget ? placeEnd(arrow.endBinding!, endTarget, rawEnd, last > 1 ? world[last - 1] : rawStart, gap) : world[last];
  if (same(start, world[0]) && same(end, world[last])) return arrow;
  world[0] = start; world[last] = end;
  return {
    ...arrow, x: start.x, y: start.y, width: end.x - start.x, height: end.y - start.y,
    points: world.map(p => ({ x: p.x - start.x, y: p.y - start.y })),
  };
}

const indexes = new WeakMap<readonly CanvasElement[], Map<string, CanvasElement>>();
function indexOf(elements: readonly CanvasElement[]) {
  let index = indexes.get(elements);
  if (!index) indexes.set(elements, index = new Map(elements.map(el => [el.id, el])));
  return index;
}

/** Finds elements by ID, live geometry where `live` overrides the scene. */
export function sceneLookup(elements: readonly CanvasElement[], live?: Live) {
  const index = indexOf(elements);
  return (id: string) => live?.has(id) ? live.get(id) : index.get(id);
}

/**
 * The changes plus every bound arrow they affect: an arrow whose target changed (or which changed itself) is
 * re-placed, and an end whose target the changes remove loses its binding where it is. Returns `changes` when nothing follows.
 */
export function followBindings(elements: readonly CanvasElement[], changes: ElementChanges): ElementChanges {
  if (!changes.size) return changes;
  const index = indexOf(elements);
  const lookup = (id: string) => changes.has(id) ? changes.get(id) : index.get(id);
  let out: Map<string, CanvasElement | null> | undefined;
  const follow = (id: string, arrow: CanvasElement | null | undefined) => {
    if (!arrow || arrow.type !== 'arrow' || (!arrow.startBinding && !arrow.endBinding)) return;
    const start = arrow.startBinding?.elementId, end = arrow.endBinding?.elementId;
    if (!changes.has(id) && !(start && changes.has(start)) && !(end && changes.has(end))) return;
    let next = arrow;
    if (start && changes.get(start) === null) next = withoutBinding(next, 'start');
    if (end && changes.get(end) === null) next = withoutBinding(next, 'end');
    next = resolveArrow(next, lookup);
    if (next !== arrow) (out ??= new Map(changes)).set(id, next);
  };
  for (const el of elements) follow(el.id, changes.has(el.id) ? changes.get(el.id) : el);
  for (const [id, el] of changes) if (!index.has(id)) follow(id, el);
  return out ?? changes;
}

/** Moved elements; a moved arrow keeps only the bindings whose target moves with it. */
export function moveElements(elements: readonly CanvasElement[], ids: ReadonlySet<string>, dx: number, dy: number): ElementChanges {
  const changes = new Map<string, CanvasElement>();
  for (const el of elements) {
    if (!ids.has(el.id)) continue;
    let next: CanvasElement = { ...el, x: el.x + dx, y: el.y + dy };
    if (el.startBinding && !ids.has(el.startBinding.elementId)) next = withoutBinding(next, 'start');
    if (el.endBinding && !ids.has(el.endBinding.elementId)) next = withoutBinding(next, 'end');
    changes.set(el.id, next);
  }
  return changes;
}
