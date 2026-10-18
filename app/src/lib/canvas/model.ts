export interface Point { x: number; y: number }
export type CanvasStrokeStyle = 'solid' | 'dashed' | 'dotted';
export type CanvasFontFamily = 'sans' | 'serif' | 'mono';
export type CanvasTextAlign = 'left' | 'center' | 'right';
export type CanvasVerticalAlign = 'top' | 'middle' | 'bottom';
export type CanvasArrowhead = 'none' | 'arrow' | 'triangle' | 'dot' | 'bar' | 'diamond';
export type CanvasRoundness = 'sharp' | 'round';
/** How an arrow's path runs through its points: straight segments, a smooth curve, or right angles. */
export type CanvasArrowType = 'sharp' | 'curved' | 'elbow';
/** An edge anchor is normalised 0..1 within the target's bounds. */
export type CanvasBinding = { elementId: string; mode: 'center' } | { elementId: string; mode: 'edge'; anchor: Point };
/** An elbow segment the user placed, local to the arrow's x/y: the vertical one on the line at `x`, or the horizontal one at `y`. */
export type CanvasFixedSegment = { x: number } | { y: number };
export interface CanvasViewport { x: number; y: number; zoom: number; [key: string]: unknown }

/** Extension fields are retained when reading and saving a document. */
export interface CanvasElement {
  id: string;
  type: 'rectangle' | 'ellipse' | 'diamond' | 'line' | 'arrow' | 'freedraw' | 'text';
  x: number;
  y: number;
  width: number;
  height: number;
  points?: Point[];
  text?: string;
  fontSize?: number;
  stroke: string;
  fill: string;
  strokeWidth: number;
  style: 'sketch' | 'clean';
  seed: number;
  opacity?: number;
  strokeStyle?: CanvasStrokeStyle;
  fontFamily?: CanvasFontFamily;
  textAlign?: CanvasTextAlign;
  verticalAlign?: CanvasVerticalAlign;
  startArrowhead?: CanvasArrowhead;
  endArrowhead?: CanvasArrowhead;
  roundness?: CanvasRoundness;
  arrowType?: CanvasArrowType;
  startBinding?: CanvasBinding;
  endBinding?: CanvasBinding;
  /** An elbow arrow's placed segments in path order (`elbow/lanes.ts`). */
  fixedSegments?: CanvasFixedSegment[];
  [key: string]: unknown;
}

/** What an absent optional field means; shape labels centre where text starts left, rectangles round where diamonds are sharp. */
export const CANVAS_DEFAULTS = {
  opacity: 1, strokeStyle: 'solid', fontFamily: 'sans', fontSize: 24, textAlign: 'left', labelAlign: 'center',
  verticalAlign: 'middle', startArrowhead: 'none', endArrowhead: 'arrow', roundness: 'round', diamondRoundness: 'sharp',
  arrowType: 'sharp',
} as const satisfies Record<string, string | number>;

export const LABELLED_TYPES: ReadonlySet<CanvasElement['type']> = new Set(['rectangle', 'ellipse', 'diamond']);
export const hasLabel = (el: CanvasElement) => LABELLED_TYPES.has(el.type) && Boolean(el.text?.trim());
export const textAlignOf = (el: CanvasElement): CanvasTextAlign => el.textAlign ?? (el.type === 'text' ? CANVAS_DEFAULTS.textAlign : CANVAS_DEFAULTS.labelAlign);
export const roundnessOf = (el: CanvasElement): CanvasRoundness =>
  el.roundness ?? (el.type === 'diamond' ? CANVAS_DEFAULTS.diamondRoundness : CANVAS_DEFAULTS.roundness);
export const arrowTypeOf = (el: CanvasElement): CanvasArrowType => el.arrowType ?? CANVAS_DEFAULTS.arrowType;
export const arrowheadOf = (el: CanvasElement, end: 'start' | 'end'): CanvasArrowhead =>
  end === 'start' ? el.startArrowhead ?? CANVAS_DEFAULTS.startArrowhead : el.endArrowhead ?? CANVAS_DEFAULTS.endArrowhead;

export type CanvasTool = CanvasElement['type'] | 'select' | 'hand' | 'eraser';

export interface CanvasDocument {
  type: 'oculus-canvas';
  version: 1;
  elements: CanvasElement[];
  viewport: CanvasViewport;
  background: string;
  [key: string]: unknown;
}

export function createCanvasDocument(): CanvasDocument {
  return { type: 'oculus-canvas', version: 1, elements: [], viewport: { x: 0, y: 0, zoom: 1 }, background: '#ffffff' };
}

const TYPES = new Set(['rectangle', 'ellipse', 'diamond', 'line', 'arrow', 'freedraw', 'text']);
const record = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const unit = (v: unknown) => finite(v) && v >= 0 && v <= 1;
const ARROWHEADS = ['none', 'arrow', 'triangle', 'dot', 'bar', 'diamond'];
const CHOICES: Record<string, readonly string[]> = {
  strokeStyle: ['solid', 'dashed', 'dotted'], fontFamily: ['sans', 'serif', 'mono'], textAlign: ['left', 'center', 'right'],
  verticalAlign: ['top', 'middle', 'bottom'], startArrowhead: ARROWHEADS, endArrowhead: ARROWHEADS, roundness: ['sharp', 'round'],
  arrowType: ['sharp', 'curved', 'elbow'],
};
/** Target IDs are not checked here: a dangling binding must not block loading. */
const binding = (v: unknown) => record(v) && typeof v.elementId === 'string' && v.elementId !== ''
  && (v.mode === 'center' || (v.mode === 'edge' && record(v.anchor) && unit(v.anchor.x) && unit(v.anchor.y)));
const fixedSegment = (v: unknown) => record(v) && Object.keys(v).length === 1 && (finite(v.x) || finite(v.y));

function validate(value: unknown): asserts value is CanvasDocument {
  if (!record(value) || value.type !== 'oculus-canvas') throw new Error('This file is not an Oculus canvas.');
  if (value.version !== 1) throw new Error(`Unsupported canvas version: ${String(value.version)}.`);
  if (!Array.isArray(value.elements)) throw new Error('Canvas elements must be an array.');
  if (!record(value.viewport) || !finite(value.viewport.x) || !finite(value.viewport.y) || !finite(value.viewport.zoom) || value.viewport.zoom <= 0) {
    throw new Error('Canvas viewport must have finite coordinates and a positive zoom.');
  }
  if (typeof value.background !== 'string') throw new Error('Canvas background must be a colour.');
  const ids = new Set<string>();
  for (const [i, el] of value.elements.entries()) {
    if (!record(el) || typeof el.id !== 'string' || !el.id || ids.has(el.id)) throw new Error(`Canvas element ${i + 1} needs a unique ID.`);
    ids.add(el.id);
    if (typeof el.type !== 'string' || !TYPES.has(el.type)) throw new Error(`Unsupported canvas element type: ${String(el.type)}.`);
    for (const key of ['x', 'y', 'width', 'height', 'strokeWidth', 'seed']) {
      if (!finite(el[key])) throw new Error(`Canvas element ${i + 1} has an invalid ${key}.`);
    }
    if ((el.strokeWidth as number) < 0 || typeof el.stroke !== 'string' || typeof el.fill !== 'string' || (el.style !== 'sketch' && el.style !== 'clean')) {
      throw new Error(`Canvas element ${i + 1} has an invalid appearance.`);
    }
    if (el.fontSize !== undefined && (!finite(el.fontSize) || el.fontSize <= 0)) throw new Error(`Canvas element ${i + 1} has an invalid font size.`);
    if (el.text !== undefined && typeof el.text !== 'string') throw new Error(`Canvas element ${i + 1} has invalid text.`);
    if (el.points !== undefined && (!Array.isArray(el.points) || !el.points.every(p => record(p) && finite(p.x) && finite(p.y)))) {
      throw new Error(`Canvas element ${i + 1} has invalid points.`);
    }
    if (el.opacity !== undefined && !unit(el.opacity)) throw new Error(`Canvas element ${i + 1} has an invalid opacity.`);
    for (const [key, values] of Object.entries(CHOICES)) {
      if (el[key] !== undefined && !values.includes(el[key] as string)) throw new Error(`Canvas element ${i + 1} has an invalid ${key}.`);
    }
    for (const key of ['startBinding', 'endBinding']) {
      if (el[key] !== undefined && !binding(el[key])) throw new Error(`Canvas element ${i + 1} has an invalid ${key}.`);
    }
    if (el.fixedSegments !== undefined && (!Array.isArray(el.fixedSegments) || !el.fixedSegments.every(fixedSegment))) {
      throw new Error(`Canvas element ${i + 1} has invalid fixed segments.`);
    }
  }
}

export function parseCanvasDocument(text: string): CanvasDocument {
  let value: unknown;
  try { value = JSON.parse(text); } catch { throw new Error('Canvas file contains invalid JSON.'); }
  validate(value);
  return value;
}

export function serializeCanvasDocument(document: CanvasDocument): string {
  validate(document);
  return `${JSON.stringify(document, null, 2)}\n`;
}
