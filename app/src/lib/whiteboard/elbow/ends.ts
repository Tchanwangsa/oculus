import { isBindable, anchorPoint, outlineCenter, outlineMidpoints } from '../outline';
import { elementBounds, type Bounds } from '../geometry';
import type { CanvasBinding, CanvasElement, Point } from '../model';
import { worldPoints } from '../arrow/points';
import { STEP, sideHeading, vectorHeading, type Heading } from './headings';

export type Lookup = (id: string) => CanvasElement | null | undefined;

/** Where a route starts or ends, the direction it leaves that point in, and the bound target's bounds (absent for a free end). */
export interface RouteEnd { point: Point; heading: Heading; box?: Bounds }

/**
 * An elbow arrow's two ends. A bound end sits on a side of its target, pulled out along the side's outward normal by
 * 4 + strokeWidth/2: an edge binding at its anchor, on the side the anchor is on; a centre binding at the midpoint of
 * the side facing the other end, or, when the other end has an edge on the same shape, the next side clockwise (facing
 * it would put both ends on one point). A free end stays where it is and heads along the dominant axis toward the other end.
 */
const bindingTarget = (binding: CanvasBinding | undefined, lookup: Lookup) => {
  const el = binding ? lookup(binding.elementId) : undefined;
  return el && isBindable(el) ? el : undefined;
};

/** A bound end on `heading`'s side of `el`, from `raw` on its outline, pulled out by the arrow's gap. */
const place = (arrow: CanvasElement, el: CanvasElement, raw: Point, heading: Heading): RouteEnd => {
  const gap = 4 + arrow.strokeWidth / 2;
  return { point: { x: raw.x + STEP[heading].x * gap, y: raw.y + STEP[heading].y * gap }, heading, box: elementBounds(el) };
};

export function elbowEnds(arrow: CanvasElement, lookup: Lookup): [RouteEnd, RouteEnd] {
  const free = worldPoints(arrow);
  const target = (binding?: CanvasBinding) => bindingTarget(binding, lookup);
  const startTarget = target(arrow.startBinding), endTarget = target(arrow.endBinding);
  const sideOf = (el: CanvasElement, p: Point) => {
    const c = outlineCenter(el);
    return sideHeading(elementBounds(el), p.x - c.x, p.y - c.y);
  };
  const bound = (binding: CanvasBinding, el: CanvasElement, other: { binding?: CanvasBinding; el?: CanvasElement; point: Point }): RouteEnd => {
    if (binding.mode === 'edge') {
      const raw = anchorPoint(el, binding.anchor);
      return place(arrow, el, raw, sideOf(el, raw));
    }
    if (other.el === el && other.binding?.mode === 'edge') {
      const heading = ((sideOf(el, anchorPoint(el, other.binding.anchor)) + 1) % 4) as Heading;
      return place(arrow, el, outlineMidpoints(el)[heading], heading);
    }
    const toward = !other.el || !other.binding ? other.point : other.binding.mode === 'edge' ? anchorPoint(other.el, other.binding.anchor) : outlineCenter(other.el);
    const heading = sideOf(el, toward);
    return place(arrow, el, outlineMidpoints(el)[heading], heading);
  };
  const first = free[0], last = free[free.length - 1];
  const start = startTarget && bound(arrow.startBinding!, startTarget, { binding: arrow.endBinding, el: endTarget, point: last });
  const end = endTarget && bound(arrow.endBinding!, endTarget, { binding: arrow.startBinding, el: startTarget, point: first });
  const startPoint = start?.point ?? first, endPoint = end?.point ?? last;
  return [
    start ?? { point: first, heading: vectorHeading(endPoint.x - first.x, endPoint.y - first.y) },
    end ?? { point: last, heading: vectorHeading(startPoint.x - last.x, startPoint.y - last.y) },
  ];
}

/**
 * The ends a route may use: what `elbowEnds` places, then, for a centre binding, its other three sides clockwise.
 * With fixed segments the facing side can be the wrong one (a fixed line on the far side), so routing tries them all.
 */
export function elbowEndChoices(arrow: CanvasElement, lookup: Lookup): [RouteEnd[], RouteEnd[]] {
  const ends = elbowEnds(arrow, lookup);
  const choices = (chosen: RouteEnd, binding: CanvasBinding | undefined, other: RouteEnd): RouteEnd[] => {
    const el = binding?.mode === 'center' ? bindingTarget(binding, lookup) : undefined;
    if (!el || !chosen.box) return [chosen];
    const sides = [1, 2, 3].map(k => ((chosen.heading + k) % 4) as Heading);
    const others = sides.map(h => place(arrow, el, outlineMidpoints(el)[h], h));
    return [chosen, ...others.filter(end => end.point.x !== other.point.x || end.point.y !== other.point.y)];
  };
  return [choices(ends[0], arrow.startBinding, ends[1]), choices(ends[1], arrow.endBinding, ends[0])];
}
