import { arrowTypeOf, type CanvasElement } from '../model';
import { routeElbow, type Lookup } from '../elbow';
import { withoutFixedSegments } from '../elbow/lanes';
import { throughPoints, worldPoints } from './points';

/**
 * After an arrow type change: becoming elbow routes the arrow between its ends (finding targets with `lookup`);
 * leaving elbow keeps only the two ends, since its bends were routed rather than placed; sharp and curved share their points.
 * Any change of type drops fixed segments.
 */
export function retypeArrow(next: CanvasElement, previous: CanvasElement, lookup: Lookup = () => undefined): CanvasElement {
  if (next.type !== 'arrow') return next;
  const was = arrowTypeOf(previous), now = arrowTypeOf(next);
  if (was !== now) next = withoutFixedSegments(next);
  if (now === 'elbow') return was === 'elbow' ? next : routeElbow(next, lookup);
  if (was !== 'elbow') return next;
  const points = worldPoints(next);
  return points.length > 2 ? throughPoints(next, [points[0], points[points.length - 1]]) : next;
}
