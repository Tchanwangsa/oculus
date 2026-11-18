import type { Point } from '../model';
import { boxOf, segmentEntersBox } from './grid';
import { matchLanes, type Lane } from './lanes';
import { elbowRoute } from './route';
import { CROSSING } from './astar';
import type { RouteEnd } from './ends';

/** What crossing a target's body costs a candidate, in bends: more than any detour round it. */
const THROUGH_TARGET = 10;

/** A route's cost as the search counts it: its bends, crossed lanes and any target it runs through, then its length. */
function score(route: readonly Point[], start: RouteEnd, end: RouteEnd, lanes: readonly Lane[]): [number, number] {
  const bodies = [start.box, end.box].filter(box => !!box).map(box => boxOf(box!));
  let length = 0, through = false;
  for (let i = 1; i < route.length; i++) {
    length += Math.abs(route[i].x - route[i - 1].x) + Math.abs(route[i].y - route[i - 1].y);
    through ||= bodies.some(box => segmentEntersBox(route[i - 1], route[i], box));
  }
  const crossed = lanes.length - matchLanes(route, lanes).filter(k => k !== undefined).length;
  return [Math.max(0, route.length - 2) + CROSSING * crossed + (through ? THROUGH_TARGET : 0), length];
}

/**
 * The cheapest route over every pairing of the ends' choices (`elbowEndChoices`), each along the lanes; the first
 * pairing (the sides `elbowEnds` would pick) wins ties.
 */
export function cheapestRoute(starts: readonly RouteEnd[], ends: readonly RouteEnd[], lanes: readonly Lane[]): Point[] {
  let best: { route: Point[]; cost: [number, number] } | undefined;
  for (const start of starts) {
    for (const end of ends) {
      const route = elbowRoute(start, end, lanes), cost = score(route, start, end, lanes);
      if (!best || cost[0] < best.cost[0] || (cost[0] === best.cost[0] && cost[1] < best.cost[1])) best = { route, cost };
    }
  }
  return best!.route;
}
