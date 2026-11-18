import type { CanvasElement, Point } from '../model';
import { throughPoints, worldPoints } from '../arrow/points';
import { elbowEndChoices, elbowEnds, type Lookup } from './ends';
import { elbowRoute } from './route';
import { routeFits } from './fits';
import { elementLanes, type Lane } from './lanes';
import { cheapestRoute } from './sides';

export type { Lookup } from './ends';

/** The arrow through `route`, or the arrow itself when that is where it already runs. */
function placeRoute(arrow: CanvasElement, route: readonly Point[]): CanvasElement {
  const current = worldPoints(arrow);
  const unchanged = route.length === current.length && route.every((p, i) => Math.abs(p.x - current[i].x) < 1e-9 && Math.abs(p.y - current[i].y) < 1e-9);
  return unchanged ? arrow : throughPoints(arrow, route);
}

/**
 * Elbow arrows: a right-angled route between the arrow's ends that leaves and enters each bound target square to the
 * side it is bound to and keeps clear of both targets. Bound ends are placed from their bindings; a free end keeps its
 * point. Returns the arrow itself when the route is unchanged, so a re-route that moves nothing is not an edit.
 *
 * An arrow with fixed segments keeps its points while they still fit its ends (`routeFits`), so a dragged segment
 * stays as dragged until an end moves; then it is routed along its fixed segments' lines, a centre-bound end leaving
 * by whichever side gives the cheapest route (`cheapestRoute`).
 */
export function routeElbow(arrow: CanvasElement, lookup: Lookup): CanvasElement {
  const lanes = elementLanes(arrow);
  if (!lanes.length) {
    const [start, end] = elbowEnds(arrow, lookup);
    return placeRoute(arrow, elbowRoute(start, end));
  }
  const [starts, ends] = elbowEndChoices(arrow, lookup), points = worldPoints(arrow);
  if (starts.some(start => ends.some(end => routeFits(points, start, end, lanes)))) return arrow;
  return placeRoute(arrow, cheapestRoute(starts, ends, lanes));
}

/** Routes the arrow afresh even where its points still fit, as after releasing a fixed segment. */
export function rerouteElbow(arrow: CanvasElement, lookup: Lookup): CanvasElement {
  const lanes: Lane[] = elementLanes(arrow);
  if (!lanes.length) return placeRoute(arrow, elbowRoute(...elbowEnds(arrow, lookup)));
  return placeRoute(arrow, cheapestRoute(...elbowEndChoices(arrow, lookup), lanes));
}
