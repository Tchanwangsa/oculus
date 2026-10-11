/** A rectangle in some frame's CSS px (`layout.ts`), edges not size. */
export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export const union = (a: Box, b: Box): Box => ({
  left: Math.min(a.left, b.left),
  top: Math.min(a.top, b.top),
  right: Math.max(a.right, b.right),
  bottom: Math.max(a.bottom, b.bottom),
});

export const area = (b: Box): number => (b.right - b.left) * (b.bottom - b.top);

/** Whether `b` holds the point, its edges pushed out by `reach`. */
export const holds = (b: Box, x: number, y: number, reach = 0): boolean =>
  x >= b.left - reach && x <= b.right + reach && y >= b.top - reach && y <= b.bottom + reach;
