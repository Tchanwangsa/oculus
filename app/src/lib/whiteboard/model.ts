import { isWhiteboardImageDataUrl } from './imageSource';
import { paperProblem, type Paper } from './paper';

export interface Point { x: number; y: number }
export type WhiteboardStrokeStyle = 'solid' | 'dashed' | 'dotted';
export type WhiteboardFontFamily = 'sans' | 'serif' | 'mono';
export type WhiteboardTextAlign = 'left' | 'center' | 'right';
export type WhiteboardVerticalAlign = 'top' | 'middle' | 'bottom';
export type WhiteboardArrowhead = 'none' | 'arrow' | 'triangle' | 'dot' | 'bar' | 'diamond';
export type WhiteboardRoundness = 'sharp' | 'round';
/** What a freehand stroke is drawn with (`pens.ts`); the two markers are the highlighter's. */
export type WhiteboardPen = 'ball' | 'fountain' | 'pencil' | 'brush' | 'sketch' | 'chinese' | 'dashed' | 'dotted' | 'roundMarker' | 'flatMarker';
/** How an arrow's path runs through its points: straight segments, a smooth curve, or right angles. */
export type WhiteboardArrowType = 'sharp' | 'curved' | 'elbow';
/** An edge anchor is normalised 0..1 within the target's bounds. */
export type WhiteboardBinding = { elementId: string; mode: 'center' } | { elementId: string; mode: 'edge'; anchor: Point };
/** An elbow segment the user placed, local to the arrow's x/y: the vertical one on the line at `x`, or the horizontal one at `y`. */
export type WhiteboardFixedSegment = { x: number } | { y: number };
export interface WhiteboardViewport { x: number; y: number; zoom: number; [key: string]: unknown }

/** Extension fields are retained when reading and saving a document. */
export interface WhiteboardElement {
  id: string;
  type: 'rectangle' | 'ellipse' | 'diamond' | 'line' | 'arrow' | 'freedraw' | 'text' | 'image';
  x: number;
  y: number;
  width: number;
  height: number;
  points?: Point[];
  text?: string;
  fontSize?: number;
  /** Embedded raster bytes keep copied elements and saved whiteboards self-contained. */
  dataUrl?: string;
  sourceName?: string;
  naturalWidth?: number;
  naturalHeight?: number;
  stroke: string;
  fill: string;
  strokeWidth: number;
  style: 'sketch' | 'clean';
  seed: number;
  opacity?: number;
  strokeStyle?: WhiteboardStrokeStyle;
  fontFamily?: WhiteboardFontFamily;
  textAlign?: WhiteboardTextAlign;
  verticalAlign?: WhiteboardVerticalAlign;
  startArrowhead?: WhiteboardArrowhead;
  endArrowhead?: WhiteboardArrowhead;
  roundness?: WhiteboardRoundness;
  arrowType?: WhiteboardArrowType;
  pen?: WhiteboardPen;
  startBinding?: WhiteboardBinding;
  endBinding?: WhiteboardBinding;
  /** An elbow arrow's placed segments in path order (`elbow/lanes.ts`). */
  fixedSegments?: WhiteboardFixedSegment[];
  /** A backdrop (an assignment's page image): drawn, but never selected, moved or erased (`paper.ts`). */
  locked?: boolean;
  [key: string]: unknown;
}

/** What an absent optional field means; shape labels centre where text starts left, rectangles round where diamonds are sharp. */
export const WHITEBOARD_DEFAULTS = {
  opacity: 1, strokeStyle: 'solid', fontFamily: 'sans', fontSize: 24, textAlign: 'left', labelAlign: 'center',
  verticalAlign: 'middle', startArrowhead: 'none', endArrowhead: 'arrow', roundness: 'round', diamondRoundness: 'sharp',
  arrowType: 'sharp', pen: 'ball',
} as const satisfies Record<string, string | number>;

export const LABELLED_TYPES: ReadonlySet<WhiteboardElement['type']> = new Set(['rectangle', 'ellipse', 'diamond']);
export const hasLabel = (el: WhiteboardElement) => LABELLED_TYPES.has(el.type) && Boolean(el.text?.trim());
export const textAlignOf = (el: WhiteboardElement): WhiteboardTextAlign => el.textAlign ?? (el.type === 'text' ? WHITEBOARD_DEFAULTS.textAlign : WHITEBOARD_DEFAULTS.labelAlign);
export const roundnessOf = (el: WhiteboardElement): WhiteboardRoundness =>
  el.roundness ?? (el.type === 'diamond' ? WHITEBOARD_DEFAULTS.diamondRoundness : WHITEBOARD_DEFAULTS.roundness);
export const arrowTypeOf = (el: WhiteboardElement): WhiteboardArrowType => el.arrowType ?? WHITEBOARD_DEFAULTS.arrowType;
export const arrowheadOf = (el: WhiteboardElement, end: 'start' | 'end'): WhiteboardArrowhead =>
  end === 'start' ? el.startArrowhead ?? WHITEBOARD_DEFAULTS.startArrowhead : el.endArrowhead ?? WHITEBOARD_DEFAULTS.endArrowhead;

/** The highlighter draws freehand strokes with a marker pen. */
export type WhiteboardTool = Exclude<WhiteboardElement['type'], 'image'> | 'highlighter' | 'select' | 'lasso' | 'hand' | 'eraser' | 'capture' | 'crop';

export interface WhiteboardDocument {
  type: 'oculus-whiteboard';
  version: 1;
  elements: WhiteboardElement[];
  viewport: WhiteboardViewport;
  background: string;
  /** A notebook's pages, stacked down the board. A `.whiteboard` may hold one too: kept as written, never drawn (`notebookPaper`). */
  paper?: Paper;
  [key: string]: unknown;
}

export function createWhiteboardDocument(): WhiteboardDocument {
  return { type: 'oculus-whiteboard', version: 1, elements: [], viewport: { x: 0, y: 0, zoom: 1 }, background: '#ffffff' };
}

const TYPES = new Set(['rectangle', 'ellipse', 'diamond', 'line', 'arrow', 'freedraw', 'text', 'image']);
const record = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const unit = (v: unknown) => finite(v) && v >= 0 && v <= 1;
const ARROWHEADS = ['none', 'arrow', 'triangle', 'dot', 'bar', 'diamond'];
const CHOICES: Record<string, readonly string[]> = {
  strokeStyle: ['solid', 'dashed', 'dotted'], fontFamily: ['sans', 'serif', 'mono'], textAlign: ['left', 'center', 'right'],
  verticalAlign: ['top', 'middle', 'bottom'], startArrowhead: ARROWHEADS, endArrowhead: ARROWHEADS, roundness: ['sharp', 'round'],
  arrowType: ['sharp', 'curved', 'elbow'],
  pen: ['ball', 'fountain', 'pencil', 'brush', 'sketch', 'chinese', 'dashed', 'dotted', 'roundMarker', 'flatMarker'],
};
/** Target IDs are not checked here: a dangling binding must not block loading. */
const binding = (v: unknown) => record(v) && typeof v.elementId === 'string' && v.elementId !== ''
  && (v.mode === 'center' || (v.mode === 'edge' && record(v.anchor) && unit(v.anchor.x) && unit(v.anchor.y)));
const fixedSegment = (v: unknown) => record(v) && Object.keys(v).length === 1 && (finite(v.x) || finite(v.y));

/** The type older files carry. Parsing reads it as the current type, so the next save writes the new one. */
const LEGACY_TYPE = 'oculus-canvas';

function validate(value: unknown): asserts value is WhiteboardDocument {
  if (!record(value) || value.type !== 'oculus-whiteboard') throw new Error('This file is not an Oculus whiteboard.');
  if (value.version !== 1) throw new Error(`Unsupported whiteboard version: ${String(value.version)}.`);
  if (!Array.isArray(value.elements)) throw new Error('Whiteboard elements must be an array.');
  if (!record(value.viewport) || !finite(value.viewport.x) || !finite(value.viewport.y) || !finite(value.viewport.zoom) || value.viewport.zoom <= 0) {
    throw new Error('Whiteboard viewport must have finite coordinates and a positive zoom.');
  }
  if (typeof value.background !== 'string') throw new Error('Whiteboard background must be a colour.');
  if (value.paper !== undefined) {
    const problem = paperProblem(value.paper);
    if (problem) throw new Error(problem);
  }
  const ids = new Set<string>();
  for (const [i, el] of value.elements.entries()) {
    if (!record(el) || typeof el.id !== 'string' || !el.id || ids.has(el.id)) throw new Error(`Whiteboard element ${i + 1} needs a unique ID.`);
    ids.add(el.id);
    if (typeof el.type !== 'string' || !TYPES.has(el.type)) throw new Error(`Unsupported whiteboard element type: ${String(el.type)}.`);
    for (const key of ['x', 'y', 'width', 'height', 'strokeWidth', 'seed']) {
      if (!finite(el[key])) throw new Error(`Whiteboard element ${i + 1} has an invalid ${key}.`);
    }
    if ((el.strokeWidth as number) < 0 || typeof el.stroke !== 'string' || typeof el.fill !== 'string' || (el.style !== 'sketch' && el.style !== 'clean')) {
      throw new Error(`Whiteboard element ${i + 1} has an invalid appearance.`);
    }
    if (el.fontSize !== undefined && (!finite(el.fontSize) || el.fontSize <= 0)) throw new Error(`Whiteboard element ${i + 1} has an invalid font size.`);
    if (el.text !== undefined && typeof el.text !== 'string') throw new Error(`Whiteboard element ${i + 1} has invalid text.`);
    if ((el.type === 'image' || el.dataUrl !== undefined) && !isWhiteboardImageDataUrl(el.dataUrl)) throw new Error(`Whiteboard element ${i + 1} needs an embedded PNG, JPEG, WebP or GIF image.`);
    if (el.sourceName !== undefined && typeof el.sourceName !== 'string') throw new Error(`Whiteboard element ${i + 1} has an invalid image source name.`);
    for (const key of ['naturalWidth', 'naturalHeight']) {
      if (el[key] !== undefined && (!finite(el[key]) || el[key] <= 0)) throw new Error(`Whiteboard element ${i + 1} has an invalid ${key}.`);
    }
    if (el.points !== undefined && (!Array.isArray(el.points) || !el.points.every(p => record(p) && finite(p.x) && finite(p.y)))) {
      throw new Error(`Whiteboard element ${i + 1} has invalid points.`);
    }
    if (el.opacity !== undefined && !unit(el.opacity)) throw new Error(`Whiteboard element ${i + 1} has an invalid opacity.`);
    if (el.locked !== undefined && typeof el.locked !== 'boolean') throw new Error(`Whiteboard element ${i + 1} has an invalid locked flag.`);
    for (const [key, values] of Object.entries(CHOICES)) {
      if (el[key] !== undefined && !values.includes(el[key] as string)) throw new Error(`Whiteboard element ${i + 1} has an invalid ${key}.`);
    }
    for (const key of ['startBinding', 'endBinding']) {
      if (el[key] !== undefined && !binding(el[key])) throw new Error(`Whiteboard element ${i + 1} has an invalid ${key}.`);
    }
    if (el.fixedSegments !== undefined && (!Array.isArray(el.fixedSegments) || !el.fixedSegments.every(fixedSegment))) {
      throw new Error(`Whiteboard element ${i + 1} has invalid fixed segments.`);
    }
  }
}

export function parseWhiteboardDocument(text: string): WhiteboardDocument {
  let value: unknown;
  try { value = JSON.parse(text); } catch { throw new Error('Whiteboard file contains invalid JSON.'); }
  if (record(value) && value.type === LEGACY_TYPE) value.type = 'oculus-whiteboard';
  validate(value);
  return value;
}

export function serializeWhiteboardDocument(document: WhiteboardDocument): string {
  validate(document);
  return `${JSON.stringify(document, null, 2)}\n`;
}
