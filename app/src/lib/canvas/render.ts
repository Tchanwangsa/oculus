import { CANVAS_DEFAULTS, hasLabel, LABELLED_TYPES, roundnessOf, type CanvasDocument, type CanvasElement, type Point } from './model';
import { diamondCorners, diamondVertices, elementArrowheads, elementBounds, rectangleRadius, selectionBounds, strokeDash, type Bounds } from './geometry';
import { linePath } from './arrow/path';
import { pointHandles } from './arrow/handles';
import { canvasFont, layoutShapeLabel, layoutTextElement, type TextLayout } from './renderText';
import { visibleElements } from './engine';
import { outlineCenter, outlineMidpoints } from './outline';
import type { BindingSnap } from './bindings';

export interface CanvasPaintOptions {
  width: number;
  height: number;
  pixelRatio: number;
  grid: boolean;
  selected: Set<string>;
  selectionColor: string;
  marquee?: Bounds;
  /** Text being edited is hidden; a shape being labelled draws without its label, which the overlay replaces. */
  editingId?: string;
  overrides?: Map<string, CanvasElement | null>;
  /** The shape an arrow end would bind to while one is drawn or dragged; never exported. */
  bindingHint?: BindingSnap | null;
}

interface ElementPaths { body: Path2D; heads?: Path2D; filledHeads?: Path2D }
const pathCache = new WeakMap<CanvasElement, ElementPaths>();
let labelCache = new WeakMap<CanvasElement, TextLayout>();
// Labels measured before the web font arrived wrap with fallback widths.
if (typeof document !== 'undefined') document.fonts?.addEventListener('loadingdone', () => { labelCache = new WeakMap(); });

function elementPaths(element: CanvasElement): ElementPaths {
  const cached = pathCache.get(element);
  if (cached) return cached;
  const path = new Path2D(), paths: ElementPaths = { body: path };
  const { width: w, height: h } = element;
  if (element.type === 'rectangle') path.roundRect(Math.min(0, w), Math.min(0, h), Math.abs(w), Math.abs(h), rectangleRadius(element));
  if (element.type === 'ellipse') path.ellipse(w / 2, h / 2, Math.abs(w / 2), Math.abs(h / 2), 0, 0, Math.PI * 2);
  if (element.type === 'diamond' && roundnessOf(element) === 'round') {
    diamondCorners(w, h).forEach(({ from, vertex, to }, i) => {
      if (i) path.lineTo(from.x, from.y); else path.moveTo(from.x, from.y);
      path.quadraticCurveTo(vertex.x, vertex.y, to.x, to.y);
    });
    path.closePath();
  } else if (element.type === 'diamond') {
    diamondVertices(w, h).forEach((p, i) => i ? path.lineTo(p.x, p.y) : path.moveTo(p.x, p.y));
    path.closePath();
  }
  if (element.type === 'freedraw' || element.type === 'line' || element.type === 'arrow') {
    const { points, curves } = linePath(element);
    path.moveTo(points[0].x, points[0].y);
    if (element.type === 'freedraw' && points.length > 2) {
      for (let i = 1; i < points.length - 1; i++) {
        const next = points[i + 1];
        path.quadraticCurveTo(points[i].x, points[i].y, (points[i].x + next.x) / 2, (points[i].y + next.y) / 2);
      }
      const last = points[points.length - 1]; path.lineTo(last.x, last.y);
    } else if (curves.length) for (const { c1, c2, to } of curves) path.bezierCurveTo(c1.x, c1.y, c2.x, c2.y, to.x, to.y);
    else for (const point of points.slice(1)) path.lineTo(point.x, point.y);
    for (const head of elementArrowheads(element)) {
      const target = head.closed || head.dot ? (paths.filledHeads ??= new Path2D()) : (paths.heads ??= new Path2D());
      if (head.dot) { target.moveTo(head.dot.center.x + head.dot.radius, head.dot.center.y); target.arc(head.dot.center.x, head.dot.center.y, head.dot.radius, 0, Math.PI * 2); }
      head.points.forEach((p, i) => i ? target.lineTo(p.x, p.y) : target.moveTo(p.x, p.y));
      if (head.closed) target.closePath();
    }
  }
  pathCache.set(element, paths);
  return paths;
}

function strokeOutline(context: CanvasRenderingContext2D, element: CanvasElement, paths: ElementPaths) {
  context.setLineDash(strokeDash(element)); context.stroke(paths.body); context.setLineDash([]);
  if (paths.heads) context.stroke(paths.heads);
  if (paths.filledHeads) context.stroke(paths.filledHeads);
}

function drawLines(context: CanvasRenderingContext2D, element: CanvasElement, layout: TextLayout) {
  context.font = canvasFont(layout.fontSize, layout.fontFamily);
  context.textAlign = layout.textAlign;
  context.textBaseline = 'top';
  context.fillStyle = element.stroke;
  for (const line of layout.lines) context.fillText(line.text, line.x, line.y);
}

export function drawElement(context: CanvasRenderingContext2D, element: CanvasElement, label = true) {
  context.save();
  context.translate(element.x, element.y);
  const opacity = element.opacity ?? CANVAS_DEFAULTS.opacity;
  context.globalAlpha = opacity;
  context.strokeStyle = element.stroke;
  context.fillStyle = element.fill;
  context.lineWidth = element.strokeWidth;
  context.lineCap = 'round';
  context.lineJoin = 'round';
  if (element.type === 'text') drawLines(context, element, layoutTextElement(element));
  else {
    const paths = elementPaths(element);
    if (element.fill !== 'transparent' && LABELLED_TYPES.has(element.type)) context.fill(paths.body);
    if (paths.filledHeads) { context.fillStyle = element.stroke; context.fill(paths.filledHeads); }
    strokeOutline(context, element, paths);
    if (element.style === 'sketch' && element.type !== 'freedraw') {
      context.save();
      context.translate(Math.sin(element.seed * 1.618) * 1.1, Math.cos(element.seed) * 0.9);
      context.rotate(Math.sin(element.seed) * 0.003);
      context.globalAlpha = opacity * 0.55;
      context.lineWidth = element.strokeWidth * 0.65;
      strokeOutline(context, element, paths);
      context.restore();
    }
    if (label && hasLabel(element)) {
      let layout = labelCache.get(element);
      if (!layout) labelCache.set(element, layout = layoutShapeLabel(element));
      drawLines(context, element, layout);
    }
  }
  context.restore();
}

export function resizeHandles(bounds: Bounds): Point[] {
  return [
    { x: bounds.x, y: bounds.y }, { x: bounds.x + bounds.width, y: bounds.y },
    { x: bounds.x + bounds.width, y: bounds.y + bounds.height }, { x: bounds.x, y: bounds.y + bounds.height },
  ];
}

/** A single selected line or arrow edits its points (`pointHandles`) instead of resizing. */
export const pointEditTarget = (elements: readonly CanvasElement[]) =>
  elements.length === 1 && (elements[0].type === 'line' || elements[0].type === 'arrow') ? elements[0] : undefined;

export function elementResizeHandles(elements: CanvasElement[], bounds: Bounds): Point[] {
  return pointEditTarget(elements) ? [] : resizeHandles(bounds);
}

/** The dashed frame drawn round a selection; a single line or arrow has none, only its point handles. */
export const selectionFrame = (elements: CanvasElement[]): Bounds | null =>
  pointEditTarget(elements) ? null : selectionBounds(elements);

/** Points as the resize handles' squares; midpoints and elbow segment handles smaller, round and faint. */
function paintPointHandles(context: CanvasRenderingContext2D, element: CanvasElement, background: string, zoom: number) {
  for (const handle of pointHandles(element, zoom)) {
    context.beginPath();
    if (handle.kind === 'point') {
      context.fillStyle = background;
      context.roundRect(handle.at.x - 4 / zoom, handle.at.y - 4 / zoom, 8 / zoom, 8 / zoom, 2 / zoom);
      context.fill(); context.stroke();
    } else {
      // Midpoint and segment handles are soft brand dots, so they read as "drag me" without competing with the points.
      context.fillStyle = context.strokeStyle; context.globalAlpha = 0.4;
      context.arc(handle.at.x, handle.at.y, 5.5 / zoom, 0, Math.PI * 2);
      context.fill(); context.globalAlpha = 1;
    }
  }
}

/** The target's outline in the brand colour with its snap points; the active one is filled. Sizes stay constant on screen. */
function paintBindingHint(context: CanvasRenderingContext2D, { target, binding, point }: BindingSnap, color: string, background: string, zoom: number) {
  context.save();
  context.strokeStyle = color;
  context.globalAlpha = 0.45;
  context.lineWidth = target.strokeWidth + 4 / zoom;
  context.lineJoin = 'round';
  if (target.type === 'text') {
    const b = elementBounds(target);
    context.strokeRect(b.x, b.y, b.width, b.height);
  } else {
    context.translate(target.x, target.y); context.stroke(elementPaths(target).body); context.translate(-target.x, -target.y);
  }
  context.globalAlpha = 1;
  context.lineWidth = 1.5 / zoom;
  const dot = (p: Point, active: boolean) => {
    context.beginPath(); context.arc(p.x, p.y, (active ? 4.5 : 3.5) / zoom, 0, Math.PI * 2);
    context.fillStyle = active ? color : background; context.fill(); context.stroke();
  };
  const snaps = [outlineCenter(target), ...outlineMidpoints(target)];
  const activeIndex = binding.mode === 'center' ? 0 : snaps.findIndex((p, i) => i > 0 && Math.hypot(p.x - point.x, p.y - point.y) < 1e-6);
  snaps.forEach((p, i) => dot(p, i === activeIndex));
  if (activeIndex < 0) dot(point, true);
  context.restore();
}

export function paintCanvas(context: CanvasRenderingContext2D, document: CanvasDocument, options: CanvasPaintOptions) {
  const { width, height, pixelRatio, grid, selected, selectionColor } = options;
  const { x, y, zoom } = document.viewport;
  context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  context.clearRect(0, 0, width, height);
  context.fillStyle = document.background;
  context.fillRect(0, 0, width, height);
  if (grid) {
    let step = 24 * zoom;
    while (step < 16) step *= 4;
    context.fillStyle = selectionColor;
    context.globalAlpha = 0.17;
    for (let gx = ((x % step) + step) % step; gx < width; gx += step) {
      for (let gy = ((y % step) + step) % step; gy < height; gy += step) {
        context.beginPath(); context.arc(gx, gy, 0.8, 0, Math.PI * 2); context.fill();
      }
    }
    context.globalAlpha = 1;
  }
  context.translate(x, y); context.scale(zoom, zoom);
  const viewport = { x: -x / zoom, y: -y / zoom, width: width / zoom, height: height / zoom };
  const draw = (element: CanvasElement) => {
    if (element.id !== options.editingId) drawElement(context, element);
    else if (element.type !== 'text') drawElement(context, element, false);
  };
  for (const element of visibleElements(document.elements, viewport)) {
    if (!options.overrides?.has(element.id)) draw(element);
  }
  if (options.overrides) for (const element of options.overrides.values()) if (element) draw(element);
  if (options.bindingHint) paintBindingHint(context, options.bindingHint, selectionColor, document.background, zoom);
  context.strokeStyle = selectionColor;
  context.lineWidth = 1 / zoom;
  const selectionElements = document.elements.filter(element => selected.has(element.id) && !options.overrides?.has(element.id));
  if (options.overrides) for (const element of options.overrides.values()) {
    if (element && selected.has(element.id)) selectionElements.push(element);
  }
  const editing = pointEditTarget(selectionElements), frame = selectionFrame(selectionElements);
  if (editing) paintPointHandles(context, editing, document.background, zoom);
  if (frame) {
    const pad = 5 / zoom;
    context.setLineDash([4 / zoom, 3 / zoom]);
    context.strokeRect(frame.x - pad, frame.y - pad, frame.width + pad * 2, frame.height + pad * 2);
    context.setLineDash([]);
    for (const handle of resizeHandles(frame)) {
      context.fillStyle = document.background;
      context.beginPath(); context.roundRect(handle.x - 4 / zoom, handle.y - 4 / zoom, 8 / zoom, 8 / zoom, 2 / zoom); context.fill(); context.stroke();
    }
  }
  if (options.marquee) {
    const bounds = options.marquee;
    context.globalAlpha = 0.1; context.fillStyle = selectionColor; context.fillRect(bounds.x, bounds.y, bounds.width, bounds.height);
    context.globalAlpha = 1; context.strokeRect(bounds.x, bounds.y, bounds.width, bounds.height);
  }
}

export function exportCanvasPng(document: CanvasDocument): Promise<Blob | null> {
  const bounds = selectionBounds(document.elements);
  if (!bounds) return Promise.resolve(null);
  const padding = 32;
  const ratio = Math.min(2, 8192 / Math.max(bounds.width + padding * 2, bounds.height + padding * 2));
  const canvas = window.document.createElement('canvas');
  canvas.width = Math.ceil((bounds.width + padding * 2) * ratio);
  canvas.height = Math.ceil((bounds.height + padding * 2) * ratio);
  const context = canvas.getContext('2d');
  if (!context) return Promise.resolve(null);
  context.scale(ratio, ratio);
  context.fillStyle = document.background; context.fillRect(0, 0, canvas.width / ratio, canvas.height / ratio);
  context.translate(padding - bounds.x, padding - bounds.y);
  for (const element of document.elements) drawElement(context, element);
  return new Promise(resolve => canvas.toBlob(resolve, 'image/png'));
}

export function selectedBounds(document: CanvasDocument, selected: Set<string>) {
  return selectionBounds(document.elements.filter(element => selected.has(element.id)));
}

export { elementBounds };
