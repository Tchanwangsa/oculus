import { add, dot, perpendicular, scale, sub, unit, type Vec3 } from "./vec";

/** Points per radian along an arc: smooth at any size the frame draws. */
const PER_RADIAN = 24;

/**
 * Points of the shorter arc around `center` from direction `from` to
 * direction `to` (vectors from the centre), `radius` from it (default
 * `|from|`). Opposite directions have no one shorter arc: it turns about z
 * (about x when `from` is vertical). The same direction, or a zero vector,
 * is no arc: an empty list.
 */
export function arcPoints(center: Vec3, from: Vec3, to: Vec3, radius?: number): Vec3[] {
  const u = unit(from);
  const t = unit(to);
  if (!u || !t) return [];
  const r = radius ?? Math.hypot(...from);
  const cos = Math.max(-1, Math.min(1, dot(u, t)));
  const angle = Math.acos(cos);
  if (angle < 1e-9 || !(r > 0)) return [];
  // The part of `to` across `u`: zero when they are opposite.
  const w = unit(sub(t, scale(u, cos))) ?? perpendicular(u);
  const steps = Math.max(2, Math.ceil(angle * PER_RADIAN));
  const points: Vec3[] = [];
  for (let i = 0; i <= steps; i++) {
    const s = (angle * i) / steps;
    points.push(add(center, scale(add(scale(u, Math.cos(s)), scale(w, Math.sin(s))), r)));
  }
  return points;
}
