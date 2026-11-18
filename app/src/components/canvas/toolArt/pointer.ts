import { GRAB_OUTLINE, GRAB_SHADE, GRAB_SILHOUETTE } from './grabTrace';
import { right } from './geometry';
import type { Part } from './types';

const WHITE_SHADE = '#c9d1dc';
const LOOP = 'M255 450C185 450 120 440 75 400C50 370 80 320 110 280C140 230 120 150 170 100C210 60 300 50 370 80C440 110 470 200 480 290';

export const select: Part[] = [
  { kind: 'shape', d: 'M130 62V392L212 315L268 432L330 404L274 290H390Z', fill: '#ffffff', shades: [[right(250), WHITE_SHADE]] },
];

export const lasso: Part[] = [
  { kind: 'marquee', d: LOOP, fill: '#d6e9ff' },
  { kind: 'shape', d: 'M270 228V433L321 385L356 458L395 440L360 370H432Z', fill: '#ffffff', shades: [[right(340), WHITE_SHADE]] },
];

const CORNER = '#5b8def';
const CORNER_SHADE = '#3f6fd1';
const corner = (d: string): Part => ({ kind: 'rope', d, width: 56, fill: CORNER, shades: [[right(256), CORNER_SHADE]] });

/** A dashed area inside four corner brackets, as a screen capture frames what it takes. */
export const capture: Part[] = [
  { kind: 'marquee', d: 'M150 150H362V362H150V150', fill: '#d6e9ff' },
  corner('M64 168V64H168'), corner('M344 64H448V168'), corner('M448 344V448H344'), corner('M168 448H64V344'),
];

export const grab: Part[] = [
  { kind: 'trace', silhouette: GRAB_SILHOUETTE, outline: GRAB_OUTLINE, fill: '#ffd2b0', shades: [[GRAB_SHADE, '#eab088']] },
];
