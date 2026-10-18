import type { Bounds } from '../geometry';
import type { Point } from '../model';

/** The four directions a route segment can run, in the order the grid visits neighbours and `outlineMidpoints` lists sides. */
export type Heading = 0 | 1 | 2 | 3;
export const UP = 0, RIGHT = 1, DOWN = 2, LEFT = 3;
export const STEP: readonly Point[] = [{ x: 0, y: -1 }, { x: 1, y: 0 }, { x: 0, y: 1 }, { x: -1, y: 0 }];
export const flip = (h: Heading) => ((h + 2) % 4) as Heading;
export const isHorizontal = (h: Heading) => h === RIGHT || h === LEFT;

/** A vector's dominant axis; horizontal on a tie, and a zero vector heads right. */
export function vectorHeading(dx: number, dy: number): Heading {
  if (Math.abs(dx) >= Math.abs(dy)) return dx < 0 ? LEFT : RIGHT;
  return dy < 0 ? UP : DOWN;
}

/**
 * The side of `box` that a ray from its centre along (dx, dy) leaves through: the dominant axis once each is scaled
 * to the box. Compared cross-multiplied so a zero-size side needs no division; a zero-size box uses the raw vector.
 */
export function sideHeading(box: Bounds, dx: number, dy: number): Heading {
  const across = Math.abs(dx) * box.height, down = Math.abs(dy) * box.width;
  if (across === 0 && down === 0) return vectorHeading(dx, dy);
  if (across >= down) return dx < 0 ? LEFT : RIGHT;
  return dy < 0 ? UP : DOWN;
}
