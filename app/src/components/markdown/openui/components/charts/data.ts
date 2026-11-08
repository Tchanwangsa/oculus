/** Coercers for model data. While a fence streams, arrays arrive with holes
 *  and half-written objects, so every reader drops what is not finite. */

export type Point = { x: number; y: number };
export type SeriesData = { label: string; points: Point[] };

export const finite = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);

export function numbers(raw: unknown): number[] {
  return Array.isArray(raw) ? raw.slice(0, 400).filter(finite) : [];
}

export function points(raw: unknown): Point[] {
  if (!Array.isArray(raw)) return [];
  return raw.slice(0, 400).flatMap((p, i) => {
    const x = finite(p) ? i : p?.x;
    const y = finite(p) ? p : p?.y;
    return finite(x) && finite(y) ? [{ x, y }] : [];
  });
}

export function strings(raw: unknown): string[] {
  return Array.isArray(raw) ? raw.map((s) => (typeof s === "string" ? s : s == null ? "" : String(s))) : [];
}

/** At most five series, matching the app's fixed categorical palette. */
export const MAX_SERIES = 5;

/** `Series` nodes, as the runtime hands them to a chart. */
export function seriesList(raw: unknown): SeriesData[] {
  if (!Array.isArray(raw)) return [];
  if (raw.some((p) => finite(p) || (p && typeof p === "object" && "x" in p))) return [{ label: "Value", points: points(raw) }];
  return raw.slice(0, MAX_SERIES).flatMap((node, i) => {
    const props = (node as { props?: { label?: unknown; points?: unknown } } | null)?.props;
    if (!props) return [];
    return [{ label: typeof props.label === "string" ? props.label : `Series ${i + 1}`, points: points(props.points) }];
  });
}

export const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : NaN);

export type Bin = { x0: number; x1: number; count: number };

/** Equal-width bins over raw values; `count` defaults to √n within 5–30. */
export function binValues(values: number[], count?: unknown): Bin[] {
  if (!values.length) return [];
  const n = finite(count) && count >= 1 ? Math.min(60, Math.round(count)) : Math.min(30, Math.max(5, Math.ceil(Math.sqrt(values.length))));
  let lo = Math.min(...values);
  let hi = Math.max(...values);
  if (lo === hi) { lo -= 0.5; hi += 0.5; }
  const width = (hi - lo) / n;
  const bins = Array.from({ length: n }, (_, i) => ({ x0: lo + i * width, x1: lo + (i + 1) * width, count: 0 }));
  for (const v of values) bins[Math.min(n - 1, Math.floor((v - lo) / width))].count++;
  return bins;
}

/** Pre-binned `{x, y}` points (bin centres and counts) as bins, each as wide
 *  as the narrowest gap between centres. */
export function binsFromPoints(pts: Point[]): Bin[] {
  const sorted = [...pts].sort((a, b) => a.x - b.x);
  let gap = Infinity;
  for (let i = 1; i < sorted.length; i++) if (sorted[i].x > sorted[i - 1].x) gap = Math.min(gap, sorted[i].x - sorted[i - 1].x);
  const half = Number.isFinite(gap) ? gap / 2 : 0.5;
  return sorted.map((p) => ({ x0: p.x - half, x1: p.x + half, count: p.y }));
}

/** Area under a curve between two x values, by trapezoids. */
export function area(pts: Point[], from = -Infinity, to = Infinity): number {
  let sum = 0;
  for (let i = 1; i < pts.length; i++) {
    const a = Math.max(pts[i - 1].x, from);
    const b = Math.min(pts[i].x, to);
    if (b <= a) continue;
    const at = (x: number) => pts[i - 1].y + ((pts[i].y - pts[i - 1].y) * (x - pts[i - 1].x)) / (pts[i].x - pts[i - 1].x || 1);
    sum += ((at(a) + at(b)) / 2) * (b - a);
  }
  return sum;
}

/** Least-squares line through points, or null when x never varies. */
export function fitLine(pts: Point[]): { slope: number; intercept: number } | null {
  if (pts.length < 2) return null;
  const mx = mean(pts.map((p) => p.x));
  const my = mean(pts.map((p) => p.y));
  let sxx = 0;
  let sxy = 0;
  for (const p of pts) { sxx += (p.x - mx) ** 2; sxy += (p.x - mx) * (p.y - my); }
  if (!sxx) return null;
  const slope = sxy / sxx;
  return { slope, intercept: my - slope * mx };
}

/** Series colours in the palette's fixed order: colour follows the series'
 *  position in the program, never its rank. */
export const SERIES_FILL = ["bg-chart-1", "bg-chart-2", "bg-chart-3", "bg-chart-4", "bg-chart-5"];
export const SERIES_STROKE = ["stroke-chart-1", "stroke-chart-2", "stroke-chart-3", "stroke-chart-4", "stroke-chart-5"];
export const SERIES_SVG_FILL = ["fill-chart-1", "fill-chart-2", "fill-chart-3", "fill-chart-4", "fill-chart-5"];

/** First moment of a piecewise-linear density, integrated per segment. */
export function densityMean(pts: Point[]): number {
  const total = area(pts);
  let moment = 0;
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1];
    const b = pts[i];
    moment += (b.x - a.x) * (a.x * (2 * a.y + b.y) + b.x * (a.y + 2 * b.y)) / 6;
  }
  return total > 0 ? moment / total : NaN;
}
