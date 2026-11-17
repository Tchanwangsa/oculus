import { pageRects, paperBounds, type PageRect, type Paper, type PaperTemplate } from './paper';
import { selectionBounds, type Bounds } from './geometry';
import type { CanvasDocument } from './model';

export const PAGE_COLOR = '#ffffff';
export const RULE_COLOR = '#cfd8ea';
/** The heavier lines of graph and Cornell paper. */
export const BOLD_RULE_COLOR = '#a9b8d6';
export const DOT_COLOR = '#a9afc2';
/** Points between lines or dots: the pattern's finest spacing. */
const STEP: Record<Exclude<PaperTemplate, 'blank'>, number> = { grid: 14, ruled: 23, dotted: 14, graph: 14, cornell: 23, music: 7, isometric: 14 };
/** Ruled and Cornell paper leave this much blank above their first line; music paper above its first staff. */
const RULED_TOP = 60;
const DOT_SIZE = 2.2;
/** A pattern denser than this many screen pixels per step is too fine to read and is left out. */
const MIN_STEP_PX = 4;
/** Graph paper's every fifth line is bold. */
const GRAPH_BOLD = 5;
/** Cornell paper: the cue column's share of the width and the summary's share of the height. */
const CORNELL = { cue: 0.28, summary: 0.18 };
/** Music paper: five lines a `STEP` apart per staff, a staff every `pitch`, inside side and bottom margins. */
const MUSIC = { lines: 5, pitch: 72, side: 36, bottom: 40 };
const TAN_30 = Math.tan(Math.PI / 6), COS_30 = Math.cos(Math.PI / 6);

/** A page's template as flat coordinate lists: `lines` and `bold` are x1, y1, x2, y2 per line, `dots` is x, y per dot. */
export interface TemplateMarks { lines: number[]; bold: number[]; dots: number[] }

const overlaps = (rect: Bounds, view: Bounds) =>
  rect.x < view.x + view.width && rect.x + rect.width > view.x && rect.y < view.y + view.height && rect.y + rect.height > view.y;

/** The part of a line across the page's full width that lies between `top` and `bottom`, pushed onto `out`. */
function clipAcross(out: number[], left: number, yLeft: number, right: number, yRight: number, top: number, bottom: number) {
  if (Math.max(yLeft, yRight) < top || Math.min(yLeft, yRight) > bottom) return;
  const at = (y: number) => left + (right - left) * (y - yLeft) / (yRight - yLeft);
  const clamp = (x: number, y: number): [number, number] => y < top ? [at(top), top] : y > bottom ? [at(bottom), bottom] : [x, y];
  out.push(...clamp(left, yLeft), ...clamp(right, yRight));
}

/** The marks of `rect`'s template inside `region` (world units; the whole page by default). `zoom` drops a pattern too fine to see. */
export function templateMarks(rect: PageRect, region: Bounds = rect, zoom = 1): TemplateMarks {
  const marks: TemplateMarks = { lines: [], bold: [], dots: [] };
  const template = rect.template;
  if (template === 'blank' || STEP[template] * zoom < MIN_STEP_PX) return marks;
  const step = STEP[template];
  const left = Math.max(rect.x, region.x), right = Math.min(rect.x + rect.width, region.x + region.width);
  const top = Math.max(rect.y, region.y), bottom = Math.min(rect.y + rect.height, region.y + region.height);
  if (left >= right || top >= bottom) return marks;
  const from = (origin: number, min: number, every = step) => origin + every * Math.max(0, Math.ceil((min - origin) / every));
  const pageBottom = rect.y + rect.height, pageRight = rect.x + rect.width;
  if (template === 'dotted') {
    for (let x = from(rect.x, left); x <= right; x += step) for (let y = from(rect.y, top); y <= bottom; y += step) marks.dots.push(x, y);
  } else if (template === 'grid' || template === 'graph') {
    for (let y = from(rect.y, top); y <= bottom; y += step) marks.lines.push(left, y, right, y);
    for (let x = from(rect.x, left); x <= right; x += step) marks.lines.push(x, top, x, bottom);
    if (template === 'graph') {
      const every = step * GRAPH_BOLD;
      for (let y = from(rect.y, top, every); y <= bottom; y += every) marks.bold.push(left, y, right, y);
      for (let x = from(rect.x, left, every); x <= right; x += every) marks.bold.push(x, top, x, bottom);
    }
  } else if (template === 'ruled' || template === 'cornell') {
    const header = rect.y + RULED_TOP;
    for (let y = from(header, top); y <= bottom; y += step) marks.lines.push(left, y, right, y);
    if (template === 'cornell') {
      // The summary starts on a ruled line, so its rule never falls between two of them.
      const summary = header + step * Math.floor((rect.height * (1 - CORNELL.summary) - RULED_TOP) / step);
      const cue = rect.x + Math.round(rect.width * CORNELL.cue);
      marks.bold.push(rect.x, header, pageRight, header, rect.x, summary, pageRight, summary, cue, header, cue, summary);
    }
  } else if (template === 'music') {
    const height = step * (MUSIC.lines - 1);
    for (let staff = rect.y + RULED_TOP; staff + height <= pageBottom - MUSIC.bottom; staff += MUSIC.pitch) {
      if (staff > bottom || staff + height < top) continue;
      for (let i = 0; i < MUSIC.lines; i++) marks.lines.push(rect.x + MUSIC.side, staff + i * step, pageRight - MUSIC.side, staff + i * step);
    }
  } else {
    // Isometric: upright lines and two families at 30° to the horizontal, meeting at the corners of equal triangles.
    for (let x = from(rect.x, left, step * COS_30); x <= right; x += step * COS_30) marks.lines.push(x, top, x, bottom);
    const rise = rect.width * TAN_30;
    for (let c = rect.y - step * Math.ceil(rise / step); c <= pageBottom; c += step) clipAcross(marks.lines, rect.x, c, pageRight, c + rise, top, bottom);
    for (let c = rect.y; c <= pageBottom + rise; c += step) clipAcross(marks.lines, rect.x, c, pageRight, c - rise, top, bottom);
  }
  return marks;
}

/** One page's template, clipped to the page. `region` limits the work to what is on screen. */
export function paintTemplate(context: CanvasRenderingContext2D, rect: PageRect, zoom: number, region: Bounds = rect) {
  const { lines, bold, dots } = templateMarks(rect, region, zoom);
  if (!lines.length && !bold.length && !dots.length) return;
  context.save();
  context.beginPath(); context.rect(rect.x, rect.y, rect.width, rect.height); context.clip();
  const rule = (marks: number[], color: string, width: number) => {
    if (!marks.length) return;
    context.beginPath();
    for (let i = 0; i < marks.length; i += 4) { context.moveTo(marks[i], marks[i + 1]); context.lineTo(marks[i + 2], marks[i + 3]); }
    context.strokeStyle = color; context.lineWidth = width / zoom; context.stroke();
  };
  rule(lines, RULE_COLOR, 0.75);
  rule(bold, BOLD_RULE_COLOR, 1.1);
  if (dots.length) {
    context.beginPath();
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
    const { lines, bold, dots } = templateMarks(rect);
    const rule = (list: number[], color: string, width: number) => {
      if (!list.length) return '';
      let d = '';
      for (let i = 0; i < list.length; i += 4) d += `M${num(list[i])} ${num(list[i + 1])}L${num(list[i + 2])} ${num(list[i + 3])}`;
      return `<path d="${d}" fill="none" stroke="${color}" stroke-width="${width}"/>`;
    };
    let marks = rule(lines, RULE_COLOR, 0.75) + rule(bold, BOLD_RULE_COLOR, 1.1);
    if (dots.length) {
      let d = '';
      for (let i = 0; i < dots.length; i += 2) d += `M${num(dots[i] - DOT_SIZE / 2)} ${num(dots[i + 1] - DOT_SIZE / 2)}h${DOT_SIZE}v${DOT_SIZE}h-${DOT_SIZE}z`;
      marks += `<path d="${d}" fill="${DOT_COLOR}"/>`;
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
