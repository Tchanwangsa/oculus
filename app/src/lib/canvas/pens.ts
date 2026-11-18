import { polylineLength, polylinePoints } from './arrow/path';
import { CANVAS_DEFAULTS, type CanvasElement, type CanvasPen, type Point } from './model';

/**
 * How each pen lays ink along a freehand stroke. A line pen strokes the stroke's smoothed path, once or in a few
 * passes; a nib pen fills an outline whose width changes along the stroke. `render.ts` and `renderSvg.ts` both draw
 * from here, so the screen, the PNG and the SVG agree.
 */
export const PENS: readonly CanvasPen[] = ['ball', 'fountain', 'pencil', 'brush', 'sketch', 'chinese', 'dashed', 'dotted'];
export const MARKERS: readonly CanvasPen[] = ['roundMarker', 'flatMarker'];
export const PEN_LABELS: Record<CanvasPen, string> = {
  ball: 'Ball pen', fountain: 'Fountain pen', pencil: 'Pencil', brush: 'Brush', sketch: 'Sketch', chinese: 'Chinese brush',
  dashed: 'Dashed pen', dotted: 'Dotted pen', roundMarker: 'Rounded marker', flatMarker: 'Rectangular marker',
};
export const penOf = (element: Pick<CanvasElement, 'pen'>): CanvasPen => element.pen ?? CANVAS_DEFAULTS.pen;
export const isMarker = (pen: CanvasPen) => MARKERS.includes(pen);

export interface PenPass { dx: number; dy: number; width: number; alpha: number }
export interface PenLine { dash: number[]; cap: 'round' | 'butt'; join: 'round' | 'bevel'; passes: PenPass[] }

/** How a line pen strokes the path, or null for a nib pen, which fills `nibOutline` instead. */
export function penLine(element: CanvasElement): PenLine | null {
  const pen = penOf(element), width = element.strokeWidth, seed = element.seed;
  const line = (passes: PenPass[], dash: number[] = [], flat = false): PenLine => ({ dash, cap: flat ? 'butt' : 'round', join: flat ? 'bevel' : 'round', passes });
  const whole: PenPass = { dx: 0, dy: 0, width, alpha: 1 };
  if (pen === 'fountain' || pen === 'brush' || pen === 'chinese') return null;
  if (pen === 'dashed') return line([whole], [Math.max(6, 4 * width), Math.max(4, 3 * width)]);
  if (pen === 'dotted') return line([whole], [0, 2 * width + 2]);
  if (pen === 'flatMarker') return line([whole], [], true);
  // Graphite: two light passes a little apart, so the middle of the line is denser than its edges.
  if (pen === 'pencil') return line([{ ...whole, alpha: 0.55 }, { dx: Math.sin(seed) * width * 0.2, dy: Math.cos(seed * 1.3) * width * 0.2, width: width * 0.55, alpha: 0.5 }]);
  if (pen === 'sketch') return line([whole, { dx: Math.sin(seed * 1.618) * 1.1, dy: Math.cos(seed) * 0.9, width: width * 0.65, alpha: 0.55 }]);
  return line([whole]);
}

/** A pen whose live stroke can be painted piece by piece: one solid pass with round ends. */
export const paintsIncrementally = (element: CanvasElement) => {
  const pen = penOf(element);
  return pen === 'ball' || pen === 'roundMarker';
};

const NIB_ANGLE = Math.PI / 4;
const ramp = (value: number, from: number, to: number) => Math.min(1, Math.max(0, (value - from) / (to - from)));

/** A nib pen's width at `along` of `length` world units into the stroke, heading at `angle`. */
function nibWidth(pen: CanvasPen, base: number, along: number, length: number, angle: number): number {
  // A broad nib held at 45°: full width across it, a hairline along it.
  if (pen === 'fountain') return base * (0.3 + 0.9 * Math.abs(Math.sin(angle - NIB_ANGLE)));
  const fromEnd = length - along;
  if (pen === 'brush') {
    const taper = Math.min(length / 2, base * 8);
    return base * 1.25 * Math.min(0.3 + 0.7 * ramp(along, 0, taper), 0.12 + 0.88 * ramp(fromEnd, 0, taper));
  }
  // A loaded brush: pressed in at the start, thinning as it travels, lifted to a point.
  const t = length ? along / length : 0;
  const body = t < 0.12 ? 0.8 + 0.2 * (t / 0.12) : 1 - 0.45 * ramp(t, 0.12, 0.7);
  const lift = 0.06 + 0.94 * ramp(fromEnd, 0, Math.min(length / 2, base * 10));
  return base * 1.7 * body * lift * (0.8 + 0.2 * Math.abs(Math.sin(angle)));
}

const mid = (a: Point, b: Point) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
const MAX_SAMPLES = 6000;

/** Points along the smoothed stroke (the curve `render.ts` draws through `points`), about `step` apart. */
export function strokeSamples(points: readonly Point[], step: number): Point[] {
  if (points.length < 2) return [...points];
  const samples: Point[] = [points[0]], last = points.length - 1;
  const line = (from: Point, to: Point) => {
    const count = Math.max(1, Math.ceil(Math.hypot(to.x - from.x, to.y - from.y) / step));
    for (let k = 1; k <= count; k++) samples.push({ x: from.x + (to.x - from.x) * k / count, y: from.y + (to.y - from.y) * k / count });
  };
  let from = points[0];
  for (let i = 1; i < last; i++) {
    const bend = points[i], to = mid(bend, points[i + 1]);
    const count = Math.max(1, Math.ceil((Math.hypot(bend.x - from.x, bend.y - from.y) + Math.hypot(to.x - bend.x, to.y - bend.y)) / step));
    for (let k = 1; k <= count; k++) {
      const t = k / count, u = 1 - t;
      samples.push({ x: u * u * from.x + 2 * u * t * bend.x + t * t * to.x, y: u * u * from.y + 2 * u * t * bend.y + t * t * to.y });
    }
    from = to;
  }
  line(from, points[last]);
  return samples;
}

const n = (value: number) => Math.round(value * 100) / 100;

/**
 * A nib pen's ink as SVG path data, local to the element: a disc at every sample and a quad joining each pair, all
 * wound the same way so a nonzero fill is their union. Empty for a line pen.
 */
export function nibOutline(element: CanvasElement): string {
  if (penLine(element)) return '';
  const pen = penOf(element), base = element.strokeWidth, points = polylinePoints(element);
  if (points.length === 1) return disc(points[0], base / 2);

  // Keep room for interpolated samples and preserve the stroke's two ends.
  const stride = Math.ceil((points.length - 1) / (MAX_SAMPLES / 2 - 1));
  const controls = stride === 1 ? points : points.filter((_, i) => i % stride === 0 || i === points.length - 1);
  const step = Math.max(0.6, base / 4, polylineLength(controls) / (MAX_SAMPLES - controls.length));
  const samples = strokeSamples(controls, step);
  const lengths = [0];
  for (let i = 1; i < samples.length; i++) lengths.push(lengths[i - 1] + Math.hypot(samples[i].x - samples[i - 1].x, samples[i].y - samples[i - 1].y));
  const total = lengths[lengths.length - 1];
  // A tap has no direction: one dot of the pen's width.
  if (!total) return disc(samples[0], base / 2);
  const radii = samples.map((_, i) => {
    const a = samples[Math.max(0, i - 1)], b = samples[Math.min(samples.length - 1, i + 1)];
    return Math.max(0.15, nibWidth(pen, base, lengths[i], total, Math.atan2(b.y - a.y, b.x - a.x)) / 2);
  });
  let d = '';
  for (let i = 0; i < samples.length; i++) {
    d += disc(samples[i], radii[i]);
    if (!i) continue;
    const a = samples[i - 1], b = samples[i], ra = radii[i - 1], rb = radii[i];
    const dx = b.x - a.x, dy = b.y - a.y, length = Math.hypot(dx, dy);
    if (!length) continue;
    // The left normal first, so the quad winds with the discs (sweep 1).
    const nx = dy / length, ny = -dx / length;
    d += `M${n(a.x + nx * ra)} ${n(a.y + ny * ra)}L${n(b.x + nx * rb)} ${n(b.y + ny * rb)}L${n(b.x - nx * rb)} ${n(b.y - ny * rb)}L${n(a.x - nx * ra)} ${n(a.y - ny * ra)}Z`;
  }
  return d;
}

const disc = (center: Point, radius: number) => {
  const r = n(radius);
  return `M${n(center.x - radius)} ${n(center.y)}a${r} ${r} 0 1 1 ${n(radius * 2)} 0a${r} ${r} 0 1 1 ${n(-radius * 2)} 0Z`;
};
