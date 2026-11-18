import { below, disc, right, roundRect } from './geometry';
import type { Part } from './types';

export const text: Part[] = [
  { kind: 'shape', d: 'M116 110H396V200H300V400H212V200H116Z', fill: '#ffffff', shades: [[right(304), '#cfd5dd']] },
];

export const image: Part[] = [
  { kind: 'shape', d: roundRect(60, 96, 392, 320, 40), fill: '#a5d8ff' },
  { kind: 'shape', d: disc(170, 206, 44), fill: '#ffd23f', shades: [[right(170), '#f0b400']] },
  { kind: 'shape', d: 'M60 372L190 246L274 330L346 268L452 372V376A40 40 0 0 1 412 416H100A40 40 0 0 1 60 376Z', fill: '#4caf6a', shades: [[right(300), '#388e55']] },
  { kind: 'frame', d: roundRect(60, 96, 392, 320, 40) },
];

/** Nine squares in three rows. */
export const matrix: Part[] = [0, 1, 2].flatMap(row => [0, 1, 2].map((col): Part => {
  const x = 72 + 128 * col, y = 72 + 128 * row;
  return { kind: 'shape', d: roundRect(x, y, 112, 112, 22), fill: '#8ec5ff', shades: [[below(y + 56), '#5fa3ec']] };
}));
