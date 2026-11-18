import type { Point } from '../model';
import { isHorizontal } from './headings';
import { boxOf, clipToLanes, dongle, gridLines, padBox, pointBox, segmentEntersBox, type Box } from './grid';
import { gridRoute } from './astar';
import type { RouteEnd } from './ends';
import { runsAlong, type Lane } from './lanes';

const same = (a: Point, b: Point) => a.x === b.x && a.y === b.y;

/** Drops repeated points and the middle of any three that share an x or a y, which leaves only the corners. */
export function simplifyRoute(points: readonly Point[]): Point[] {
  const out: Point[] = [];
  for (const p of points) if (!out.length || !same(out[out.length - 1], p)) out.push(p);
  for (let i = out.length - 2; i >= 1; i--) {
    const a = out[i - 1], b = out[i], c = out[i + 1];
    if ((a.x === b.x && b.x === c.x) || (a.y === b.y && b.y === c.y)) out.splice(i, 1);
  }
  return out.length > 1 ? out : [out[0], out[0]];
}

/** Used only when no grid route exists: an L when the ends head across each other, else a Z through the midpoint. */
export function fallbackRoute(start: RouteEnd, end: RouteEnd): Point[] {
  const a = start.point, b = end.point;
  if (isHorizontal(start.heading) !== isHorizontal(end.heading)) {
    return simplifyRoute([a, isHorizontal(start.heading) ? { x: b.x, y: a.y } : { x: a.x, y: b.y }, b]);
  }
  if (isHorizontal(start.heading)) {
    const x = (a.x + b.x) / 2;
    return simplifyRoute([a, { x, y: a.y }, { x, y: b.y }, b]);
  }
  const y = (a.y + b.y) / 2;
  return simplifyRoute([a, { x: a.x, y }, { x: b.x, y }, b]);
}

/** The padded boxes a route keeps out of, one per bound end, each padded against the other end and drawn in to the lanes. */
export function routeObstacles(start: RouteEnd, end: RouteEnd, lanes: readonly Lane[] = []): [Box | undefined, Box | undefined] {
  const startBody = start.box ? boxOf(start.box) : pointBox(start.point), endBody = end.box ? boxOf(end.box) : pointBox(end.point);
  return [start.box && clipToLanes(padBox(startBody, endBody), startBody, lanes), end.box && clipToLanes(padBox(endBody, startBody), endBody, lanes)];
}

/**
 * An orthogonal route in world coordinates from one end to the other. Each bound end leaves along its heading to its
 * padded box's side (the dongle); A* joins the dongles over the grid of box edges without entering either padded box.
 * When that is impossible, two bound ends (overlapping targets) are joined without avoiding them; a free end inside
 * the other target (as while drawing out of it) takes the L/Z fallback, whose short path beats a loop round the box.
 *
 * With fixed segments (`lanes`) the route also runs along each lane in order, and a free end may turn at once, as
 * its heading was only a guess. Without a route clear of the targets it may cross them; without any, the lanes go.
 */
export function elbowRoute(start: RouteEnd, end: RouteEnd, lanes: readonly Lane[] = []): Point[] {
  if (same(start.point, end.point)) return [start.point, end.point];
  const [startBox, endBox] = routeObstacles(start, end, lanes);
  const from = dongle(start.point, start.heading, startBox), to = dongle(end.point, end.heading, endBox);
  const { xs, ys } = gridLines(startBox ?? pointBox(from), endBox ?? pointBox(to), [from, to], lanes);
  const avoid = [startBox, endBox].filter((box): box is Box => !!box);
  const fixed = lanes.length > 0;
  // A stub out of a bound end may already run along the first or last lane.
  const skipFirst = fixed && runsAlong(lanes[0], start.point, from) ? 1 : 0;
  const inner = lanes.slice(skipFirst, lanes.length - (lanes.length > skipFirst && runsAlong(lanes[lanes.length - 1], to, end.point) ? 1 : 0));
  const search = (boxes: readonly Box[]) => gridRoute({
    xs, ys, from, to, startHeading: start.heading, endHeading: end.heading,
    strictStart: same(from, start.point) && !(fixed && !start.box), strictEnd: same(to, end.point) && !(fixed && !end.box),
    blocked: (a, b) => boxes.some(box => segmentEntersBox(a, b, box)), lanes: inner,
  });
  if (fixed) {
    const path = !inner.length && same(from, to) ? [from] : search(avoid) ?? search([]);
    return path ? simplifyRoute([start.point, ...path, end.point]) : elbowRoute(start, end);
  }
  const path = same(from, to) ? [from] : search(avoid) ?? (avoid.length === 2 ? search([]) : null);
  return path ? simplifyRoute([start.point, ...path, end.point]) : fallbackRoute(start, end);
}
