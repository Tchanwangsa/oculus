import { CANVAS_DEFAULTS, hasLabel, roundnessOf, type CanvasDocument, type CanvasElement } from './model';
import { diamondCorners, diamondVertices, elementArrowheads, rectangleRadius, selectionBounds, strokeDash } from './geometry';
import { linePath } from './arrow/path';
import { CANVAS_FONT_STACKS, layoutShapeLabel, layoutTextElement, measureCanvasText, type TextLayout, type TextMeasure } from './renderText';
import { isCanvasImageDataUrl } from './imageSource';

const escape = (value: string) => value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const number = (value: number) => Math.round(value * 1000) / 1000;
const ANCHORS = { left: 'start', center: 'middle', right: 'end' } as const;

function textSvg(element: CanvasElement, layout: TextLayout): string {
  const tspans = layout.lines.map(line => `<tspan x="${number(line.x)}" y="${number(line.y)}">${escape(line.text)}</tspan>`).join('');
  return `<text fill="${escape(element.stroke)}" font-family="${escape(CANVAS_FONT_STACKS[layout.fontFamily])}" font-size="${layout.fontSize}" text-anchor="${ANCHORS[layout.textAlign]}" dominant-baseline="text-before-edge">${tspans}</text>`;
}

/** The sketch pass repeats this outline thinner and unfilled; labels are drawn once, outside it. */
function outlineSvg(element: CanvasElement, strokeWidth: number, filled: boolean): string {
  const { width: w, height: h } = element;
  const stroke = `stroke="${escape(element.stroke)}" stroke-width="${strokeWidth}" stroke-linecap="round" stroke-linejoin="round"`;
  const dash = strokeDash(element), dashed = dash.length ? ` stroke-dasharray="${dash.map(number).join(' ')}"` : '';
  const paint = `fill="${filled ? escape(element.fill) : 'none'}" ${stroke}${dashed}`;
  if (element.type === 'rectangle') {
    const radius = rectangleRadius(element);
    return `<rect x="${Math.min(0, w)}" y="${Math.min(0, h)}" width="${Math.abs(w)}" height="${Math.abs(h)}"${radius ? ` rx="${number(radius)}"` : ''} ${paint}/>`;
  }
  if (element.type === 'ellipse') return `<ellipse cx="${w / 2}" cy="${h / 2}" rx="${Math.abs(w / 2)}" ry="${Math.abs(h / 2)}" ${paint}/>`;
  if (element.type === 'diamond' && roundnessOf(element) === 'round') {
    const d = diamondCorners(w, h).map(({ from, vertex, to }, i) => `${i ? 'L' : 'M'} ${number(from.x)} ${number(from.y)} Q ${number(vertex.x)} ${number(vertex.y)} ${number(to.x)} ${number(to.y)}`).join(' ');
    return `<path d="${d} Z" ${paint}/>`;
  }
  if (element.type === 'diamond') return `<polygon points="${diamondVertices(w, h).map(p => `${p.x},${p.y}`).join(' ')}" ${paint}/>`;
  const { points, curves } = linePath(element);
  let d = `M ${number(points[0].x)} ${number(points[0].y)}`;
  if (element.type === 'freedraw' && points.length > 2) {
    for (let i = 1; i < points.length - 1; i++) d += ` Q ${number(points[i].x)} ${number(points[i].y)} ${number((points[i].x + points[i + 1].x) / 2)} ${number((points[i].y + points[i + 1].y) / 2)}`;
    const last = points[points.length - 1]; d += ` L ${number(last.x)} ${number(last.y)}`;
  } else if (curves.length) for (const { c1, c2, to } of curves) d += ` C ${number(c1.x)} ${number(c1.y)} ${number(c2.x)} ${number(c2.y)} ${number(to.x)} ${number(to.y)}`;
  else for (const point of points.slice(1)) d += ` L ${number(point.x)} ${number(point.y)}`;
  const heads = elementArrowheads(element).map(head => {
    const fill = `fill="${filled && (head.closed || head.dot) ? escape(element.stroke) : 'none'}" ${stroke}`;
    if (head.dot) return `<circle cx="${number(head.dot.center.x)}" cy="${number(head.dot.center.y)}" r="${number(head.dot.radius)}" ${fill}/>`;
    return `<path d="${head.points.map((p, i) => `${i ? 'L' : 'M'} ${number(p.x)} ${number(p.y)}`).join(' ')}${head.closed ? ' Z' : ''}" ${fill}/>`;
  });
  return `<path d="${d}" fill="none" ${stroke}${dashed}/>${heads.join('')}`;
}

function elementSvg(element: CanvasElement, measure: TextMeasure): string {
  const opacity = element.opacity ?? CANVAS_DEFAULTS.opacity;
  const group = `<g transform="translate(${element.x},${element.y})"${opacity !== 1 ? ` opacity="${number(opacity)}"` : ''}>`;
  if (element.type === 'image') {
    if (!isCanvasImageDataUrl(element.dataUrl)) throw new Error('Cannot export an image without embedded raster data.');
    return `${group}<image x="${Math.min(0, element.width)}" y="${Math.min(0, element.height)}" width="${Math.abs(element.width)}" height="${Math.abs(element.height)}" href="${escape(element.dataUrl)}" preserveAspectRatio="none"/></g>`;
  }
  if (element.type === 'text') return `${group}${textSvg(element, layoutTextElement(element))}</g>`;
  const shape = outlineSvg(element, element.strokeWidth, true);
  const sketch = element.style === 'sketch' && element.type !== 'freedraw' ? `<g opacity="0.55" transform="translate(${Math.sin(element.seed * 1.618) * 1.1},${Math.cos(element.seed) * 0.9}) rotate(${Math.sin(element.seed) * 0.003 * 180 / Math.PI})">${outlineSvg(element, element.strokeWidth * 0.65, false)}</g>` : '';
  const label = hasLabel(element) ? textSvg(element, layoutShapeLabel(element, measure)) : '';
  return `${group}${shape}${sketch}${label}</g>`;
}

/** `measure` wraps shape labels; tests pass a deterministic one. */
export function exportCanvasSvg(document: CanvasDocument, measure: TextMeasure = measureCanvasText): string {
  const bounds = selectionBounds(document.elements) ?? { x: 0, y: 0, width: 800, height: 600 };
  const x = bounds.x - 32, y = bounds.y - 32, width = bounds.width + 64, height = bounds.height + 64;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="${x} ${y} ${width} ${height}"><rect x="${x}" y="${y}" width="${width}" height="${height}" fill="${escape(document.background)}"/>${document.elements.map(element => elementSvg(element, measure)).join('')}</svg>`;
}
