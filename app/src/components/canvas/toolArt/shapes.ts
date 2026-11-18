import { disc, lowerRight, right, roundRect } from './geometry';
import type { Part } from './types';

export const rectangle: Part[] = [
  { kind: 'shape', d: roundRect(56, 110, 400, 292, 40), fill: '#5b8def', shades: [[right(340), '#3f6fd1']] },
];

export const oval: Part[] = [
  { kind: 'shape', d: 'M60 256a196 140 0 1 0 392 0a196 140 0 1 0 -392 0Z', fill: '#ef6f6c', shades: [[right(340), '#d24f4c']] },
];

export const diamond: Part[] = [
  { kind: 'shape', d: 'M256 60L452 256L256 452L60 256Z', fill: '#f6c453', shades: [[right(256), '#dba423']] },
];

export const arrow: Part[] = [
  { kind: 'rope', d: 'M96 416L312 200', width: 66, fill: '#3fb68b', shades: [[lowerRight, '#2e9672']] },
  { kind: 'shape', d: 'M430 82L381 273L239 131Z', fill: '#3fb68b', shades: [[lowerRight, '#2e9672']] },
];

const node = (cx: number, cy: number): Part => ({ kind: 'shape', d: disc(cx, cy, 50), fill: '#ffffff', shades: [[lowerRight, '#cdd3dc']] });

export const line: Part[] = [
  { kind: 'rope', d: 'M112 400L400 112', width: 56, fill: '#8a94a6', shades: [[lowerRight, '#6b7587']] },
  node(112, 400),
  node(400, 112),
];
