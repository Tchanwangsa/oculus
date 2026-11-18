import type { WhiteboardElement, WhiteboardFixedSegment, Point } from '../model';

/**
 * A fixed segment pins the line an elbow route runs along there, not its ends: a re-route keeps the line and finds
 * the segment's extent from its neighbours. In world coordinates it is a lane: the horizontal line at y = `at`, or
 * the vertical line at x = `at`. The format stores lanes local to the arrow's x/y, so moving, nudging and pasting the
 * arrow carry them along; `throughPoints` rebases them when x/y moves under fixed points, and a resize scales them.
 */
export interface Lane { horizontal: boolean; at: number }

const EPSILON = 1e-6;
const near = (a: number, b: number) => Math.abs(a - b) < EPSILON;

export function elementLanes(el: WhiteboardElement): Lane[] {
  return (el.fixedSegments ?? []).map(s => 'y' in s ? { horizontal: true, at: el.y + s.y } : { horizontal: false, at: el.x + s.x });
}

export function withoutFixedSegments(el: WhiteboardElement): WhiteboardElement {
  if (el.fixedSegments === undefined) return el;
  const { fixedSegments: _removed, ...rest } = el;
  return rest as WhiteboardElement;
}

/** Stores `lanes` as the element's fixed segments, local to its current x/y; none removes the field. */
export function withLanes(el: WhiteboardElement, lanes: readonly Lane[]): WhiteboardElement {
  if (!lanes.length) return withoutFixedSegments(el);
  return { ...el, fixedSegments: lanes.map(lane => lane.horizontal ? { y: lane.at - el.y } : { x: lane.at - el.x }) };
}

/** The same world lanes after the element's x/y moves by (dx, dy). */
export const rebaseFixedSegments = (segments: readonly WhiteboardFixedSegment[], dx: number, dy: number): WhiteboardFixedSegment[] =>
  segments.map(s => 'y' in s ? { y: s.y - dy } : { x: s.x - dx });

/** Local lanes scale with the points they sit among. */
export const scaleFixedSegments = (segments: readonly WhiteboardFixedSegment[], sx: number, sy: number): WhiteboardFixedSegment[] =>
  segments.map(s => 'y' in s ? { y: s.y * sy } : { x: s.x * sx });

/** Whether the segment from `a` to `b` runs some distance along the lane. */
export const runsAlong = (lane: Lane, a: Point, b: Point) => lane.horizontal
  ? near(a.y, lane.at) && near(b.y, lane.at) && !near(a.x, b.x)
  : near(a.x, lane.at) && near(b.x, lane.at) && !near(a.y, b.y);

/**
 * The lane each segment of a route runs along, or undefined. Lanes are matched in order, and one the route only
 * crosses (its segment has shrunk to nothing) is skipped, so the lanes after it still find their segments.
 */
export function matchLanes(points: readonly Point[], lanes: readonly Lane[]): (number | undefined)[] {
  const out: (number | undefined)[] = [];
  let next = 0;
  for (let i = 0; i + 1 < points.length; i++) {
    let found: number | undefined;
    for (let k = next; k < lanes.length && found === undefined; k++) if (runsAlong(lanes[k], points[i], points[i + 1])) found = k;
    out.push(found);
    if (found !== undefined) next = found + 1;
  }
  return out;
}
