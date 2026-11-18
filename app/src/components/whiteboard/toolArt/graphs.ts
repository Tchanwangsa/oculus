import { below, disc, lowerRight, right } from './geometry';
import type { Part } from './types';

const RED = '#ef476f';
const RED_SHADE = '#c9304f';
const GREEN = '#3fb68b';
const GREEN_SHADE = '#2e9672';
const BLUE = '#3b82f6';
const BLUE_SHADE = '#2563eb';
const origin = (cx: number, cy: number): Part => ({ kind: 'shape', d: disc(cx, cy, 40), fill: '#ffd23f', shades: [[right(cx), '#f0b400']] });

/** The two axes, x in red and y in green, meeting at the origin. */
export const xy: Part[] = [
  { kind: 'rope', d: 'M110 402H362', width: 56, fill: RED, shades: [[below(402), RED_SHADE]] },
  { kind: 'shape', d: 'M452 402L362 342V462Z', fill: RED, shades: [[below(402), RED_SHADE]] },
  { kind: 'rope', d: 'M110 402V150', width: 56, fill: GREEN, shades: [[right(110), GREEN_SHADE]] },
  { kind: 'shape', d: 'M110 60L170 150H50Z', fill: GREEN, shades: [[right(110), GREEN_SHADE]] },
  origin(110, 402),
];

/** Isometric x, y and z axes: z up, y down-right, x down-left. */
export const xyz: Part[] = [
  { kind: 'rope', d: 'M256 300V128', width: 56, fill: BLUE },
  { kind: 'rope', d: 'M256 300L357 358', width: 56, fill: GREEN },
  { kind: 'rope', d: 'M256 300L155 358', width: 56, fill: RED },
  { kind: 'shape', d: 'M256 46L316 128H196Z', fill: BLUE, shades: [[right(256), BLUE_SHADE]] },
  { kind: 'shape', d: 'M424 397L334 405L384 315Z', fill: GREEN, shades: [[lowerRight, GREEN_SHADE]] },
  { kind: 'shape', d: 'M92 395L182 401L132 315Z', fill: RED, shades: [[right(132), RED_SHADE]] },
  origin(256, 300),
];
