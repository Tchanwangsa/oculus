import { secureId } from './ids';
import type { WhiteboardElement, Point } from './model';
import { newElementStyle, type DrawingStyle } from './style';

/** Ready-made figures the rail drops at the centre of the view: a grid of squares and two sets of axes. */
export type WhiteboardTemplate = 'matrix' | 'axes2' | 'axes3';

const CELL = 64, GAP = 12, ARM = 220;
const X_AXIS = '#ef476f', Y_AXIS = '#3fb68b', Z_AXIS = '#3b82f6';
const seed = () => Math.floor(Math.random() * 100000);

/** Nine squares in three rows, in the current shape style, centred on `centre`. */
function matrix(centre: Point, style: DrawingStyle): WhiteboardElement[] {
  const span = 3 * CELL + 2 * GAP, look = newElementStyle('rectangle', style);
  return [0, 1, 2].flatMap(row => [0, 1, 2].map((col): WhiteboardElement => ({
    id: secureId(), type: 'rectangle', x: centre.x - span / 2 + col * (CELL + GAP), y: centre.y - span / 2 + row * (CELL + GAP),
    width: CELL, height: CELL, ...look, seed: seed(),
  })));
}

/** An arrow from `from` by `(dx, dy)`, in the colour of its axis. */
function axis(from: Point, dx: number, dy: number, stroke: string, style: DrawingStyle): WhiteboardElement {
  return {
    id: secureId(), type: 'arrow', x: from.x, y: from.y, width: dx, height: dy, points: [{ x: 0, y: 0 }, { x: dx, y: dy }],
    ...newElementStyle('arrow', style), stroke, seed: seed(),
  };
}

/** The x axis right and the y axis up from a shared origin, so the pair is centred on `centre`. */
function axes2(centre: Point, style: DrawingStyle): WhiteboardElement[] {
  const origin = { x: centre.x - ARM / 2, y: centre.y + ARM / 2 };
  return [axis(origin, ARM, 0, X_AXIS, style), axis(origin, 0, -ARM, Y_AXIS, style)];
}

/** Isometric axes: z up, y down and to the right, x down and to the left. */
function axes3(centre: Point, style: DrawingStyle): WhiteboardElement[] {
  const reach = ARM * 0.86, drop = ARM / 2;
  const origin = { x: centre.x, y: centre.y + ARM / 4 };
  return [axis(origin, 0, -ARM, Z_AXIS, style), axis(origin, reach * 0.866, drop * 0.5, Y_AXIS, style), axis(origin, -reach * 0.866, drop * 0.5, X_AXIS, style)];
}

export function buildTemplate(kind: WhiteboardTemplate, centre: Point, style: DrawingStyle): WhiteboardElement[] {
  return kind === 'matrix' ? matrix(centre, style) : kind === 'axes2' ? axes2(centre, style) : axes3(centre, style);
}
