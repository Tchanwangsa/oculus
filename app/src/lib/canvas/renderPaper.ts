import { pageRects, paperBounds, type PageRect, type Paper, type PaperTemplate } from './paper';
import { selectionBounds, type Bounds } from './geometry';
import type { CanvasDocument } from './model';

export const PAGE_COLOR = '#ffffff';
export const RULE_COLOR = '#cfd8ea';
export const DOT_COLOR = '#a9afc2';
/** Points between lines or dots. */
const STEP: Record<Exclude<PaperTemplate, 'blank'>, number> = { grid: 14, ruled: 23, dotted: 14 };
/** Ruled paper leaves this much blank above its first line. */
const RULED_TOP = 60;
const DOT_SIZE = 2.2;
/** A pattern denser than this many screen pixels per step is too fine to read and is left out. */
const MIN_STEP_PX = 4;

/** A page's template as flat coordinate lists: `lines` is x1, y1, x2, y2 per line, `dots` is x, y per dot. */
export interface TemplateMarks { lines: number[]; dots: number[] }

const overlaps = (rect: Bounds, view: Bounds) =>
  rect.x < view.x + view.width && rect.x + rect.width > view.x && rect.y < view.y + view.height && rect.y + rect.height > view.y;

/** The marks of `rect`'s template inside `region` (world units; the whole page by default). `zoom` drops a pattern too fine to see. */
export function templateMarks(rect: PageRect, region: Bounds = rect, zoom = 1): TemplateMarks {
  const marks: TemplateMarks = { lines: [], dots: [] };
  if (rect.template === 'blank' || STEP[rect.template] * zoom < MIN_STEP_PX) return marks;
  const step = STEP[rect.template];
  const left = Math.max(rect.x, region.x), right = Math.min(rect.x + rect.width, region.x + region.width);
  const top = Math.max(rect.y, region.y), bottom = Math.min(rect.y + rect.height, region.y + region.height);
  if (left >= right || top >= bottom) return marks;
  const from = (origin: number, min: number) => origin + step * Math.max(0, Math.ceil((min - origin) / step));
  const firstY = from(rect.template === 'ruled' ? rect.y + RULED_TOP : rect.y, top);
  const firstX = from(rect.x, left);
  if (rect.template === 'dotted') {
    for (let x = firstX; x <= right; x += step) for (let y = firstY; y <= bottom; y += step) marks.dots.push(x, y);
    return marks;
  }
  for (let y = firstY; y <= bottom; y += step) marks.lines.push(left, y, right, y);
  if (rect.template === 'grid') for (let x = firstX; x <= right; x += step) marks.lines.push(x, top, x, bottom);
  return marks;
}

/** One page's template, clipped to the page. `region` limits the work to what is on screen. */
export function paintTemplate(context: CanvasRenderingContext2D, rect: PageRect, zoom: number, region: Bounds = rect) {
  const { lines, dots } = templateMarks(rect, region, zoom);
  if (!lines.length && !dots.length) return;
  context.save();
  context.beginPath(); context.rect(rect.x, rect.y, rect.width, rect.height); context.clip();
  context.beginPath();
  if (lines.length) {
    for (let i = 0; i < lines.length; i += 4) { context.moveTo(lines[i], lines[i + 1]); context.lineTo(lines[i + 2], lines[i + 3]); }
    context.strokeStyle = RULE_COLOR; context.lineWidth = 0.75 / zoom; context.stroke();
  } else {
    for (let i = 0; i < dots.length; i += 2) context.rect(dots[i] - DOT_SIZE / 2, dots[i + 1] - DOT_SIZE / 2, DOT_SIZE, DOT_SIZE);
    context.fillStyle = DOT_COLOR; context.fill();
  }
  context.restore();
}

/** Every page on screen: a white sheet with a soft shadow on the desk, then its template. */
export function paintPaper(context: CanvasRenderingContext2D, paper: Paper, view: Bounds, zoom: number) {
  for (const rect of pageRects(paper)) {
    if (!overlaps(rect, view)) continue;
    context.save();
    context.shadowColor = 'rgba(0, 0, 0, 0.22)'; context.shadowBlur = 10; context.shadowOffsetY = 2;
    context.fillStyle = PAGE_COLOR; context.fillRect(rect.x, rect.y, rect.width, rect.height);
    context.restore();
    paintTemplate(context, rect, zoom, view);
  }
}

const num = (value: number) => Math.round(value * 100) / 100;

/** The pages as SVG: a white rectangle per page and its template as one path. */
export function paperSvg(paper: Paper): string {
  return pageRects(paper).map(rect => {
    const { lines, dots } = templateMarks(rect);
    let marks = '';
    if (lines.length) {
      let d = '';
      for (let i = 0; i < lines.length; i += 4) d += `M${num(lines[i])} ${num(lines[i + 1])}L${num(lines[i + 2])} ${num(lines[i + 3])}`;
      marks = `<path d="${d}" fill="none" stroke="${RULE_COLOR}" stroke-width="0.75"/>`;
    } else if (dots.length) {
      let d = '';
      for (let i = 0; i < dots.length; i += 2) d += `M${num(dots[i] - DOT_SIZE / 2)} ${num(dots[i + 1] - DOT_SIZE / 2)}h${DOT_SIZE}v${DOT_SIZE}h-${DOT_SIZE}z`;
      marks = `<path d="${d}" fill="${DOT_COLOR}"/>`;
    }
    return `<rect x="${rect.x}" y="${rect.y}" width="${rect.width}" height="${rect.height}" fill="${PAGE_COLOR}"/>${marks}`;
  }).join('');
}

const union = (a: Bounds, b: Bounds): Bounds => {
  const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y);
  return { x, y, width: Math.max(a.x + a.width, b.x + b.width) - x, height: Math.max(a.y + a.height, b.y + b.height) - y };
};

/** A notebook exports its pages whole, with whatever overhangs them. */
export function exportBounds(document: CanvasDocument): Bounds | null {
  const content = selectionBounds(document.elements), sheets = document.paper ? paperBounds(document.paper) : null;
  return content && sheets ? union(content, sheets) : content ?? sheets;
}
