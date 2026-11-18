import { beyond, below, right, roundRect } from './geometry';
import type { Part } from './types';

const SILVER = '#cfd4da';
const SILVER_SHADE = '#9aa3ad';
const GLOSS = '224,0 232,0 232,512 224,512';

/** A click ballpoint, drawn upright and tilted 45° into the rail's diagonal. */
export const pen: Part[] = [{
  kind: 'group', transform: 'rotate(45 256 258)', parts: [
    { kind: 'shape', d: roundRect(240, 22, 32, 50, 4), fill: '#e5484d', shades: [[right(258), '#b8323a']] },
    { kind: 'shape', d: 'M213 94H299V332H213Z', fill: '#2a4bb0', shades: [[right(268), '#1d3688'], [GLOSS, '#5f84e6']] },
    { kind: 'shape', d: roundRect(207, 70, 98, 24, 5), fill: SILVER, shades: [[right(262), SILVER_SHADE]] },
    { kind: 'shape', d: 'M213 332H299V348H213Z', fill: SILVER, shades: [[right(262), SILVER_SHADE]] },
    { kind: 'shape', d: 'M213 348H299V420H213Z', fill: '#3b3f46', shades: [[right(268), '#2a2d33'], [GLOSS, '#6a707b']] },
    { kind: 'shape', d: 'M213 420H299L268 474H244Z', fill: SILVER, shades: [[right(258), SILVER_SHADE]] },
    { kind: 'shape', d: 'M244 474H268L256 497Z', fill: '#1f2937' },
    { kind: 'shape', d: roundRect(190, 82, 36, 126, 16), fill: '#dfe3e8', shades: [[right(208), '#aab2bc']] },
  ],
}];

const GRAY = '#bcc2c9';
const GRAY_SHADE = '#9aa3ab';
const YELLOW_SHADE = '#e0a800';

export const highlighter: Part[] = [
  { kind: 'shape', d: 'M73 398L123 448L79 482L39 442Z', fill: '#ffcc00', shades: [[beyond(525), YELLOW_SHADE]] },
  { kind: 'shape', d: 'M112 284L233 412L123 448L73 398Z', fill: GRAY, shades: [[beyond(525), GRAY_SHADE]] },
  { kind: 'shape', d: 'M112 284L342 50Q372 22 404 38L470 92Q498 118 478 148L233 412Z', fill: '#ffcb05', shades: [[beyond(570), YELLOW_SHADE]] },
  { kind: 'rope', d: 'M265 254L345 173', width: 76, fill: GRAY, shades: [[beyond(525), GRAY_SHADE]] },
];

export const eraser: Part[] = [{
  kind: 'group', transform: 'rotate(-40 256 256)', parts: [
    { kind: 'shape', d: 'M112 195H262V317H112A22 22 0 0 1 90 295V217A22 22 0 0 1 112 195Z', fill: '#f58aa5', shades: [[below(256), '#e0607f']] },
    { kind: 'shape', d: 'M262 195H398A22 22 0 0 1 420 217V295A22 22 0 0 1 398 317H262Z', fill: '#4a90e2', shades: [[below(256), '#2f6bb8']] },
    { kind: 'shape', d: 'M262 195H292V317H262Z', fill: '#f5f5f5', shades: [[below(256), '#d7dce3']] },
  ],
}];
