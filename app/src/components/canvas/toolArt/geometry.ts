/** Clip polygons, in the 512 box, for the darker half of a part: right of x, below y, or below the diagonal x + y = c. */
export const right = (x: number) => `${x},0 512,0 512,512 ${x},512`;
export const below = (y: number) => `0,${y} 512,${y} 512,512 0,512`;
export const beyond = (c: number) => `${c - 512},512 512,${c - 512} 512,512`;
/** Below the anti-diagonal through the box's corners. */
export const lowerRight = '0,512 512,0 512,512';

export function roundRect(x: number, y: number, w: number, h: number, r: number) {
  const a = (px: number, py: number) => `A${r} ${r} 0 0 1 ${px} ${py}`;
  return `M${x + r} ${y}H${x + w - r}${a(x + w, y + r)}V${y + h - r}${a(x + w - r, y + h)}H${x + r}${a(x, y + h - r)}V${y + r}${a(x + r, y)}Z`;
}

export function disc(cx: number, cy: number, r: number) {
  return `M${cx - r} ${cy}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0Z`;
}
