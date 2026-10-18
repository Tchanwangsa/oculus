import type { CanvasElement, Point } from '../model';
import { polylinePoints } from './path';
import { rebaseFixedSegments } from '../elbow/lanes';

/**
 * Edits to the points of a line or arrow. Each returns a new element whose x/y
 * is its first point (local 0,0) and whose width/height reach its last, the
 * shape drawing and `resolveArrow` produce, so bindings place ends the same way.
 */
export const worldPoints = (el: CanvasElement): Point[] => polylinePoints(el).map(p => ({ x: el.x + p.x, y: el.y + p.y }));

/** The element through `world`; an elbow arrow's fixed segments stay where they are in the world. */
export function throughPoints(el: CanvasElement, world: readonly Point[]): CanvasElement {
  const start = world[0], end = world[world.length - 1];
  const next = { ...el, x: start.x, y: start.y, width: end.x - start.x, height: end.y - start.y, points: world.map(p => ({ x: p.x - start.x, y: p.y - start.y })) };
  return el.fixedSegments ? { ...next, fixedSegments: rebaseFixedSegments(el.fixedSegments, start.x - el.x, start.y - el.y) } : next;
}

/** Shift turns the segment from `anchor` to the nearest 45°, keeping its length, as drawing does. */
export function snapAngle(anchor: Point, p: Point): Point {
  const dx = p.x - anchor.x, dy = p.y - anchor.y, length = Math.hypot(dx, dy);
  const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * Math.PI / 4;
  return { x: anchor.x + Math.cos(angle) * length, y: anchor.y + Math.sin(angle) * length };
}

/** Which end a point index is, if either. */
export const endAt = (el: CanvasElement, index: number): 'start' | 'end' | undefined =>
  index === 0 ? 'start' : index === polylinePoints(el).length - 1 ? 'end' : undefined;

/** Moves one point to `world`; `snap` aligns the segment to its previous point (the next one, for the start). Every other point stays. */
export function movePoint(el: CanvasElement, index: number, world: Point, snap = false): CanvasElement {
  const points = worldPoints(el);
  const anchor = points[index === 0 ? 1 : index - 1];
  points[index] = snap && anchor ? snapAngle(anchor, world) : { x: world.x, y: world.y };
  return throughPoints(el, points);
}

/** A new point at `world` after point `segment`; it is point `segment + 1`. */
export function insertPoint(el: CanvasElement, segment: number, world: Point): CanvasElement {
  const points = worldPoints(el);
  points.splice(segment + 1, 0, { x: world.x, y: world.y });
  return throughPoints(el, points);
}

/** Removes an interior point; ends cannot be removed, so the element comes back unchanged. */
export function removePoint(el: CanvasElement, index: number): CanvasElement {
  const points = worldPoints(el);
  if (index <= 0 || index >= points.length - 1) return el;
  points.splice(index, 1);
  return throughPoints(el, points);
}
