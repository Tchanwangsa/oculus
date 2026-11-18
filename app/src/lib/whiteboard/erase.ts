/**
 * Partial erasing. Each pointer step sweeps a capsule: the segment between two samples widened by the eraser's
 * radius. A freedraw stroke it touches is subdivided once into a dense polyline (`startCut`), points the sweeps reach
 * are marked (`cutAlong`), and the unmarked runs become the stroke's pieces while the marked runs are what goes
 * (`cutPieces`). Pieces keep only the stroke's own points plus the two at each cut, so a cut adds little to the file.
 */
import { boundsIntersect, elementHitTest, segmentDistance, type Bounds } from './geometry';
import type { CanvasElement, Point } from './model';
import { polylinePoints } from './arrow/path';

export type EraserMode = 'stroke' | 'partial';
/** The partial eraser's radius in screen pixels; zoom turns it into world units. */
export const ERASER_SIZES = { small: 6, medium: 12, large: 24 } as const;
export type EraserSize = keyof typeof ERASER_SIZES;

export interface StrokeCut {
  source: CanvasElement;
  /** The stroke in world coordinates, no step longer than the cut's resolution. */
  points: Point[];
  /** 1 for the stroke's own points; subdivision points are kept only where a run ends. */
  original: Uint8Array;
  erased: Uint8Array;
  bounds: Bounds;
  /** Points outside it (the part a notebook page hides) are never erased. */
  clip?: Bounds;
}

/** `step` is the longest gap left between points, so a cut lands within `step` of the eraser's edge. */
export function startCut(source: CanvasElement, step: number, clip?: Bounds): StrokeCut {
  const local = polylinePoints(source), points: Point[] = [], original: number[] = [];
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  const push = (x: number, y: number, own: boolean) => {
    points.push({ x, y }); original.push(own ? 1 : 0);
    minX = Math.min(minX, x); minY = Math.min(minY, y); maxX = Math.max(maxX, x); maxY = Math.max(maxY, y);
  };
  local.forEach((p, i) => {
    const x = source.x + p.x, y = source.y + p.y;
    if (i) {
      const from = points[points.length - 1], parts = Math.ceil(Math.hypot(x - from.x, y - from.y) / Math.max(step, 1e-3));
      for (let k = 1; k < parts; k++) push(from.x + (x - from.x) * k / parts, from.y + (y - from.y) * k / parts, false);
    }
    push(x, y, true);
  });
  return { source, points, original: Uint8Array.from(original), erased: new Uint8Array(points.length), bounds: { x: minX, y: minY, width: maxX - minX, height: maxY - minY }, clip };
}

/** Marks every point within `radius` (plus half the stroke's width) of the segment `from`–`to`; returns whether any was new. */
export function cutAlong(cut: StrokeCut, from: Point, to: Point, radius: number): boolean {
  const reach = radius + cut.source.strokeWidth / 2;
  const sweep = { x: Math.min(from.x, to.x) - reach, y: Math.min(from.y, to.y) - reach, width: Math.abs(to.x - from.x) + reach * 2, height: Math.abs(to.y - from.y) + reach * 2 };
  if (!boundsIntersect(sweep, cut.bounds)) return false;
  const { clip } = cut;
  let changed = false;
  cut.points.forEach((p, i) => {
    if (cut.erased[i] || segmentDistance(p, from, to) > reach) return;
    if (clip && (p.x < clip.x || p.y < clip.y || p.x > clip.x + clip.width || p.y > clip.y + clip.height)) return;
    cut.erased[i] = 1; changed = true;
  });
  return changed;
}

export interface CutPieces {
  /** In stroke order; the first keeps the stroke's ID, the rest have new ones. Empty when nothing is left. */
  kept: CanvasElement[];
  /** What the eraser took, as strokes for the preview; never saved. */
  removed: CanvasElement[];
}

/** A run of the cut's points as a freedraw element normalised as a new stroke is: x/y at its first point, points local. */
function piece(source: CanvasElement, points: Point[], id: string): CanvasElement {
  const first = points[0], last = points[points.length - 1];
  return {
    ...source, id, x: first.x, y: first.y, width: last.x - first.x, height: last.y - first.y,
    points: points.map(p => ({ x: p.x - first.x, y: p.y - first.y })),
  };
}

function runs(cut: StrokeCut, erased: boolean): [number, number][] {
  const found: [number, number][] = [];
  let start = -1;
  for (let i = 0; i <= cut.points.length; i++) {
    const inside = i < cut.points.length && !!cut.erased[i] === erased;
    if (inside && start < 0) start = i;
    else if (!inside && start >= 0) { found.push([start, i - 1]); start = -1; }
  }
  return found;
}

function runPoints(cut: StrokeCut, start: number, end: number): { points: Point[]; length: number } {
  const points: Point[] = [];
  let length = 0;
  for (let i = start; i <= end; i++) {
    if (i > start) length += Math.hypot(cut.points[i].x - cut.points[i - 1].x, cut.points[i].y - cut.points[i - 1].y);
    if (cut.original[i] || i === start || i === end) points.push(cut.points[i]);
  }
  return { points, length };
}

/**
 * The stroke as the sweeps so far leave it. A kept run shorter than `minLength` world units, or half the stroke's
 * width, would draw only a nub and is dropped. Removed runs reach one point into their neighbours so the preview joins up.
 */
export function cutPieces(cut: StrokeCut, newId: () => string, minLength = 0): CutPieces {
  const shortest = Math.max(minLength, cut.source.strokeWidth / 2, 1e-9);
  const kept: CanvasElement[] = [];
  for (const [start, end] of runs(cut, false)) {
    const run = runPoints(cut, start, end);
    if (run.length >= shortest) kept.push(piece(cut.source, run.points, kept.length ? newId() : cut.source.id));
  }
  const last = cut.points.length - 1;
  const removed = runs(cut, true).map(([start, end], index) => {
    const run = runPoints(cut, Math.max(0, start - 1), Math.min(last, end + 1));
    return piece(cut.source, run.points.length > 1 ? run.points : [run.points[0], run.points[0]], `${cut.source.id}:erased:${index}`);
  });
  return { kept, removed };
}

/** Whether the eraser swept from `from` to `to` touches `element` anywhere, tested a radius apart along the way. */
export function sweepTouches(element: CanvasElement, from: Point, to: Point, radius: number): boolean {
  const steps = Math.max(1, Math.ceil(Math.hypot(to.x - from.x, to.y - from.y) / Math.max(radius, 1e-3)));
  for (let k = 0; k <= steps; k++) {
    if (elementHitTest(element, { x: from.x + (to.x - from.x) * k / steps, y: from.y + (to.y - from.y) * k / steps }, radius)) return true;
  }
  return false;
}

/** Cuts `element` along the eraser path `sweep` (one point is a dab); null when the path never reaches it. */
export function eraseStroke(element: CanvasElement, sweep: readonly Point[], radius: number, newId: () => string, options: { step?: number; minLength?: number; clip?: Bounds } = {}): CutPieces | null {
  const cut = startCut(element, options.step ?? radius / 4, options.clip);
  let touched = false;
  sweep.forEach((point, i) => { touched = cutAlong(cut, sweep[Math.max(0, i - 1)], point, radius) || touched; });
  return touched ? cutPieces(cut, newId, options.minLength) : null;
}
