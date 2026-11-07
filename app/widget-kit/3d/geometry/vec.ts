/** `[x, y, z]` in the scene's maths coordinates, z up. Plain arrays, so the
 *  geometry here runs (and is tested) without three or WebGL. */
export type Vec3 = [number, number, number];

export const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
export const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
export const scale = (a: Vec3, s: number): Vec3 => [a[0] * s, a[1] * s, a[2] * s];
export const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
export const length = (a: Vec3) => Math.hypot(a[0], a[1], a[2]);
export const finite = (a: Vec3) => Number.isFinite(a[0]) && Number.isFinite(a[1]) && Number.isFinite(a[2]);

/** `a` at unit length, or null when it has none to scale. */
export function unit(a: Vec3): Vec3 | null {
  const l = length(a);
  return l > 1e-12 && Number.isFinite(l) ? scale(a, 1 / l) : null;
}

/** A unit vector perpendicular to the unit vector `a`: across z when it can
 *  be, so a vertical axis gets a horizontal partner. */
export function perpendicular(a: Vec3): Vec3 {
  return unit(cross(a, [0, 0, 1])) ?? unit(cross(a, [1, 0, 0]))!;
}
