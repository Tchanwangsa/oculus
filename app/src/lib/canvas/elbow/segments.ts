import type { CanvasElement, Point } from '../model';
import { throughPoints, worldPoints } from '../arrow/points';
import { elbowEnds, type Lookup } from './ends';
import { STEP, vectorHeading } from './headings';
import { PADDING } from './grid';
import { simplifyRoute } from './route';
import { elementLanes, matchLanes, withLanes, type Lane } from './lanes';
import { rerouteElbow, routeElbow } from './index';

/**
 * How far a dragged first or last segment's stub runs out of a bound end before the route turns onto the moved
 * segment: PADDING, or half the segment when it is shorter than that (Excalidraw's rule). A free end needs none.
 */
const stubLength = (bound: boolean, segmentLength: number) => !bound ? 0 : segmentLength < PADDING + 5 ? segmentLength / 2 : PADDING;
/** `distance` from `end` toward `next`, which is the way a route leaves (or enters) along its end's heading. */
const stub = (end: Point, next: Point, distance: number): Point => {
  const step = STEP[vectorHeading(next.x - end.x, next.y - end.y)];
  return { x: end.x + step.x * distance, y: end.y + step.y * distance };
};

/**
 * Drags segment `segment` of an elbow arrow by `delta`, perpendicular to itself (a horizontal segment moves in y,
 * a vertical one in x), and fixes it there. Its neighbours stretch to meet it, so nothing else moves. The first or
 * last segment cannot take its bound end with it: a stub keeps leaving (or entering) the target along its heading
 * and the route turns onto the segment after it. The result is routed (`routeElbow`), which keeps these points
 * while they fit and re-routes along the fixed lines where they no longer can (the segment dragged past an end).
 */
export function moveSegment(arrow: CanvasElement, segment: number, delta: Point, lookup: Lookup): CanvasElement {
  const points = worldPoints(arrow), last = points.length - 1, a = points[segment], b = points[segment + 1];
  if (!a || !b) return arrow;
  const horizontal = a.y === b.y && a.x !== b.x;
  if (!horizontal && a.x !== b.x) return arrow;
  const at = horizontal ? a.y + delta.y : a.x + delta.x;
  const put = (p: Point): Point => horizontal ? { x: p.x, y: at } : { x: at, y: p.y };
  const [start, end] = elbowEnds(arrow, lookup), length = Math.hypot(b.x - a.x, b.y - a.y);
  const startStub = stubLength(!!start.box, length), endStub = stubLength(!!end.box, length);
  const head = segment === 0 ? [a, ...(startStub ? [stub(a, b, startStub)] : [])] : points.slice(0, segment);
  const tail = segment + 1 === last ? [...(endStub ? [stub(b, a, endStub)] : []), b] : points.slice(segment + 2);
  const from = segment === 0 ? head[head.length - 1] : a, to = segment + 1 === last ? tail[0] : b;
  const route = simplifyRoute([...head, put(from), put(to), ...tail]);
  const lanes = elementLanes(arrow), matched = matchLanes(points, lanes), lane: Lane = { horizontal, at };
  const own = matched[segment];
  if (own !== undefined) lanes[own] = lane;
  else lanes.splice(matched.slice(0, segment).reduce<number>((next, k) => k === undefined ? next : k + 1, 0), 0, lane);
  return routeElbow(throughPoints(withLanes(arrow, lanes), route), lookup);
}

/** Releases segment `segment` if it is fixed, and routes the arrow afresh along the fixed segments left. */
export function releaseSegment(arrow: CanvasElement, segment: number, lookup: Lookup): CanvasElement {
  const lanes = elementLanes(arrow), own = matchLanes(worldPoints(arrow), lanes)[segment];
  if (own === undefined) return arrow;
  return rerouteElbow(withLanes(arrow, lanes.filter((_, k) => k !== own)), lookup);
}

