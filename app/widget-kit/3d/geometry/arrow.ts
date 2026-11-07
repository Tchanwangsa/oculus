import { add, length, scale, sub, unit, type Vec3 } from "./vec";

export interface ArrowParts {
  /** Where the shaft ends and the head's base sits. */
  neck: Vec3;
  direction: Vec3;
  headLength: number;
  headRadius: number;
}

/** The head's share of a short arrow, and its width for its length. */
const HEAD_SHARE = 0.3;
const HEAD_ASPECT = 0.4;

/**
 * How an arrow from `tail` to `tip` splits into shaft and head: the head is
 * `HEAD_SHARE` of the arrow up to `maxHead` long, so a short arrow keeps its
 * proportions and a long one a head of the same size. Null for no length.
 */
export function arrowParts(tail: Vec3, tip: Vec3, maxHead: number): ArrowParts | null {
  const d = sub(tip, tail);
  const direction = unit(d);
  if (!direction) return null;
  const headLength = Math.min(length(d) * HEAD_SHARE, maxHead);
  return { neck: add(tip, scale(direction, -headLength)), direction, headLength, headRadius: headLength * HEAD_ASPECT };
}
