import { CANVAS_DEFAULTS, hasLabel, LABELLED_TYPES, roundnessOf, type CanvasDocument, type CanvasElement, type Point } from './model';
import { diamondCorners, diamondVertices, elementArrowheads, elementBounds, rectangleRadius, selectionBounds, strokeDash, type Bounds } from './geometry';
import { notebookPaper, pageRects, type PageRect, type Paper } from './paper';
import { PAGE_COLOR, clipToPage, elementPage, exportBounds, paintPaper, paintTemplate } from './renderPaper';
import { linePath } from './arrow/path';
import { pointHandles } from './arrow/handles';
import { canvasFont, LINE_HEIGHT, layoutShapeLabel, layoutTextElement, lineBaseline, type TextLayout } from './renderText';
import { visibleElements } from './engine';
import { outlineCenter, outlineMidpoints } from './outline';
import type { BindingSnap } from './bindings';
import { drawCanvasImage, loadCanvasImages } from './renderImages';
import { nibOutline, penLine } from './pens';

export interface CanvasPaintOptions {
  width: number;
  height: number;
  pixelRatio: number;
  grid: boolean;
  selected: Set<string>;
  selectionColor: string;
  marquee?: Bounds;
  /** A lasso loop being drawn, in world units. */
  lasso?: readonly Point[];
  cropSource?: Bounds;
  captureSelection?: boolean;
  captureGhost?: { snapshot: CanvasDocument; from: Bounds; to: Bounds };
  /** Text being edited is hidden; a shape being labelled draws without its label, which the overlay replaces. */
  editingId?: string;
  /** The freehand override currently painted on the ink layer. */
  inkId?: string;
  overrides?: Map<string, CanvasElement | null>;
  /** The shape an arrow end would bind to while one is drawn or dragged; never exported. */
  bindingHint?: BindingSnap | null;
  /** A notebook paints its pages and clips each element to its own; a canvas is a whiteboard, whatever `paper` it holds. */
  notebook?: boolean;
  /** What an erase in progress will remove, painted faint (`GHOST_ALPHA`) until the release commits it. */
  ghosts?: readonly CanvasElement[];
  /** The partial eraser's outline, in world units. */
  eraser?: { center: Point; radius: number };
  /** Bounds of the elements a selection box would add if released now, outlined lighter than a selection. */
  pendingSelection?: readonly Bounds[];
}

/** A ghost's share of its element's opacity. */
export const GHOST_ALPHA = 0.25;

export interface CanvasRenderOptions { notebook?: boolean }

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

const nibCache = new WeakMap<CanvasElement, Path2D>();
/** A freehand stroke in its pen: a nib pen's filled outline, or a line pen's passes over the smoothed path. */
function drawInk(context: CanvasRenderingContext2D, element: CanvasElement, opacity: number) {
  const line = penLine(element);
  if (!line) {
    let outline = nibCache.get(element);
    if (!outline) nibCache.set(element, outline = new Path2D(nibOutline(element)));
    context.fillStyle = element.stroke;
    context.fill(outline, 'nonzero');
    return;
  }
  const { body } = elementPaths(element);
  context.lineCap = line.cap; context.lineJoin = line.join;
  context.setLineDash(line.dash);
  for (const pass of line.passes) {
    context.globalAlpha = opacity * pass.alpha;
    context.lineWidth = pass.width;
    if (pass.dx || pass.dy) { context.save(); context.translate(pass.dx, pass.dy); context.stroke(body); context.restore(); }
    else context.stroke(body);
  }
  context.setLineDash([]);
}

function strokeOutline(context: CanvasRenderingContext2D, element: CanvasElement, paths: ElementPaths) {
  context.setLineDash(strokeDash(element)); context.stroke(paths.body); context.setLineDash([]);
  if (paths.heads) context.stroke(paths.heads);
  if (paths.filledHeads) context.stroke(paths.filledHeads);
}

function drawLines(context: CanvasRenderingContext2D, element: CanvasElement, layout: TextLayout) {
  context.font = canvasFont(layout.fontSize, layout.fontFamily);
  context.textAlign = layout.textAlign;
  context.textBaseline = 'alphabetic';
  context.fillStyle = element.stroke;
  // Measured at the on-screen size, so the glyphs sit exactly where the text editor's textarea puts them.
  const zoom = context.getTransform().a / (globalThis.devicePixelRatio || 1), top = layout.fontSize * (LINE_HEIGHT - 1) / 2;
  const baseline = lineBaseline(layout.fontSize * zoom, layout.fontFamily) / zoom;
  for (const line of layout.lines) context.fillText(line.text, line.x, line.y - top + baseline);
}

/** `alpha` scales the element's own opacity (a ghost). */
export function drawElement(context: CanvasRenderingContext2D, element: CanvasElement, label = true, alpha = 1) {
  context.save();
  context.translate(element.x, element.y);
  const opacity = (element.opacity ?? CANVAS_DEFAULTS.opacity) * alpha;
  context.globalAlpha = opacity;
  context.strokeStyle = element.stroke;
  context.fillStyle = element.fill;
  context.lineWidth = element.strokeWidth;
  context.lineCap = 'round';
  context.lineJoin = 'round';
  if (element.type === 'image') drawCanvasImage(context, element);
  else if (element.type === 'text') drawLines(context, element, layoutTextElement(element));
  else if (element.type === 'freedraw') drawInk(context, element, opacity);
  else {
    const paths = elementPaths(element);
    if (element.fill !== 'transparent' && LABELLED_TYPES.has(element.type)) context.fill(paths.body);
    if (paths.filledHeads) { context.fillStyle = element.stroke; context.fill(paths.filledHeads); }
    strokeOutline(context, element, paths);
    if (element.style === 'sketch') {
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

/**
 * Draws `elements` in order. With `paper` each is clipped to its page (`elementPage`), and a run of elements on one page
 * shares a clip; without it they are drawn as they are.
 */
export function drawElementsOnPages(context: CanvasRenderingContext2D, elements: Iterable<CanvasElement>, paper: Paper | undefined, draw = (element: CanvasElement) => drawElement(context, element)) {
  if (!paper) { for (const element of elements) draw(element); return; }
  let page: PageRect | null = null;
  for (const element of elements) {
    const owner = elementPage(paper, element);
    if (owner !== page) {
      if (page) context.restore();
      context.save(); clipToPage(context, owner);
      page = owner;
    }
    draw(element);
  }
  if (page) context.restore();
}

export function resizeHandles(bounds: Bounds): Point[] {
  return [
    { x: bounds.x, y: bounds.y }, { x: bounds.x + bounds.width, y: bounds.y },
    { x: bounds.x + bounds.width, y: bounds.y + bounds.height }, { x: bounds.x, y: bounds.y + bounds.height },
    { x: bounds.x + bounds.width / 2, y: bounds.y }, { x: bounds.x + bounds.width, y: bounds.y + bounds.height / 2 },
    { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height }, { x: bounds.x, y: bounds.y + bounds.height / 2 },
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

/** A thin ring, white under grey, so it reads over ink and paper alike. */
function paintEraser(context: CanvasRenderingContext2D, center: Point, radius: number, zoom: number) {
  context.save();
  context.beginPath(); context.arc(center.x, center.y, radius, 0, Math.PI * 2);
  context.strokeStyle = 'rgba(255, 255, 255, 0.9)'; context.lineWidth = 3 / zoom; context.stroke();
  context.strokeStyle = 'rgba(60, 60, 67, 0.75)'; context.lineWidth = 1 / zoom; context.stroke();
  context.restore();
}

export function paintCanvas(context: CanvasRenderingContext2D, document: CanvasDocument, options: CanvasPaintOptions) {
  const { width, height, pixelRatio, grid, selected, selectionColor } = options;
  const { x, y, zoom } = document.viewport;
  context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  context.clearRect(0, 0, width, height);
  context.fillStyle = document.background;
  context.fillRect(0, 0, width, height);
  const paper = notebookPaper(document, options.notebook);
  if (grid && !paper) {
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
  if (paper) paintPaper(context, paper, viewport, zoom);
  const draw = (element: CanvasElement) => {
    if (element.id !== options.editingId) drawElement(context, element);
    else if (element.type !== 'text') drawElement(context, element, false);
  };
  const scene = visibleElements(document.elements, viewport);
  drawElementsOnPages(context, options.overrides?.size ? scene.filter(element => !options.overrides!.has(element.id)) : scene, paper, draw);
  if (options.ghosts?.length) drawElementsOnPages(context, options.ghosts, paper, element => drawElement(context, element, true, GHOST_ALPHA));
  if (options.overrides) drawElementsOnPages(context, [...options.overrides.values()].filter((element): element is CanvasElement => !!element && element.id !== options.inkId), paper, draw);
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
  if (options.pendingSelection?.length) {
    const pad = 3 / zoom;
    context.globalAlpha = 0.55;
    for (const b of options.pendingSelection) context.strokeRect(b.x - pad, b.y - pad, b.width + pad * 2, b.height + pad * 2);
    context.globalAlpha = 1;
  }
  if (options.lasso && options.lasso.length > 1) {
    context.beginPath();
    options.lasso.forEach((point, i) => i ? context.lineTo(point.x, point.y) : context.moveTo(point.x, point.y));
    context.closePath();
    context.globalAlpha = 0.08; context.fillStyle = selectionColor; context.fill('evenodd');
    context.globalAlpha = 1; context.setLineDash([5 / zoom, 4 / zoom]); context.stroke(); context.setLineDash([]);
  }
  if (options.eraser) paintEraser(context, options.eraser.center, options.eraser.radius, zoom);
  if (options.captureGhost) {
    const { snapshot, from, to } = options.captureGhost;
    context.save(); context.fillStyle = snapshot.background;
    context.shadowColor = 'rgba(0, 0, 0, 0.3)'; context.shadowBlur = 18 * pixelRatio; context.shadowOffsetY = 5 * pixelRatio;
    context.fillRect(to.x, to.y, to.width, to.height); context.restore();
    context.save(); context.beginPath(); context.rect(to.x, to.y, to.width, to.height); context.clip();
    context.translate(to.x - from.x, to.y - from.y);
    context.fillStyle = snapshot.background; context.fillRect(from.x, from.y, from.width, from.height);
    const pages = notebookPaper(snapshot, options.notebook);
    if (pages) paintPaper(context, pages, from, zoom);
    drawElementsOnPages(context, visibleElements(snapshot.elements, from), pages);
    context.restore();
  }
  if (options.marquee) {
    const bounds = options.marquee;
    const source = options.cropSource;
    if (options.captureSelection) {
      context.save(); context.beginPath();
      context.rect(viewport.x, viewport.y, viewport.width, viewport.height);
      context.rect(bounds.x, bounds.y, bounds.width, bounds.height);
      context.clip('evenodd'); context.fillStyle = '#000000'; context.globalAlpha = 0.22;
      context.fillRect(viewport.x, viewport.y, viewport.width, viewport.height);
      context.restore();
    }
    if (source) {
      context.fillStyle = '#000000'; context.globalAlpha = 0.35;
      context.fillRect(source.x, source.y, source.width, Math.max(0, bounds.y - source.y));
      context.fillRect(source.x, bounds.y + bounds.height, source.width, Math.max(0, source.y + source.height - bounds.y - bounds.height));
      context.fillRect(source.x, bounds.y, Math.max(0, bounds.x - source.x), bounds.height);
      context.fillRect(bounds.x + bounds.width, bounds.y, Math.max(0, source.x + source.width - bounds.x - bounds.width), bounds.height);
    } else if (!options.captureSelection) { context.globalAlpha = 0.1; context.fillStyle = selectionColor; context.fillRect(bounds.x, bounds.y, bounds.width, bounds.height); }
    context.globalAlpha = 1;
    if (options.captureSelection) context.setLineDash([5 / zoom, 4 / zoom]);
    context.strokeRect(bounds.x, bounds.y, bounds.width, bounds.height); context.setLineDash([]);
    if (options.captureSelection) {
      for (const handle of resizeHandles(bounds)) {
        context.fillStyle = document.background; context.lineWidth = 1.5 / zoom;
        context.beginPath(); context.roundRect(handle.x - 3.5 / zoom, handle.y - 3.5 / zoom, 7 / zoom, 7 / zoom, 1 / zoom); context.fill(); context.stroke();
      }
    }
    if (source) {
      const handles = resizeHandles(bounds), length = Math.min(16 / zoom, bounds.width / 4, bounds.height / 4);
      context.beginPath();
      for (let index = 0; index < 4; index++) {
        const corner = handles[index], dx = index === 0 || index === 3 ? 1 : -1, dy = index < 2 ? 1 : -1;
        context.moveTo(corner.x + dx * length, corner.y); context.lineTo(corner.x, corner.y); context.lineTo(corner.x, corner.y + dy * length);
      }
      for (let index = 4; index < 8; index++) {
        const edge = handles[index], horizontal = index === 4 || index === 6;
        context.moveTo(edge.x - (horizontal ? length / 2 : 0), edge.y - (horizontal ? 0 : length / 2));
        context.lineTo(edge.x + (horizontal ? length / 2 : 0), edge.y + (horizontal ? 0 : length / 2));
      }
      context.lineWidth = 5 / zoom; context.strokeStyle = '#000000'; context.stroke();
      context.lineWidth = 3 / zoom; context.strokeStyle = '#ffffff'; context.stroke();
      context.globalAlpha = 0.4; context.lineWidth = 1 / zoom; context.beginPath();
      for (const fraction of [1 / 3, 2 / 3]) {
        context.moveTo(bounds.x + bounds.width * fraction, bounds.y); context.lineTo(bounds.x + bounds.width * fraction, bounds.y + bounds.height);
        context.moveTo(bounds.x, bounds.y + bounds.height * fraction); context.lineTo(bounds.x + bounds.width, bounds.y + bounds.height * fraction);
      }
      context.stroke(); context.globalAlpha = 1;
    }
  }
}

/** The whole drawing as a PNG; a notebook (`options.notebook`) is its pages, each element clipped to its own. */
export async function exportCanvasPng(document: CanvasDocument, options: CanvasRenderOptions = {}): Promise<Blob | null> {
  const paper = notebookPaper(document, options.notebook);
  const bounds = exportBounds(document, paper);
  if (!bounds) return null;
  await loadCanvasImages(document.elements);
  const padding = 32;
  const ratio = Math.min(2, 8192 / Math.max(bounds.width + padding * 2, bounds.height + padding * 2));
  const canvas = window.document.createElement('canvas');
  canvas.width = Math.ceil((bounds.width + padding * 2) * ratio);
  canvas.height = Math.ceil((bounds.height + padding * 2) * ratio);
  const context = canvas.getContext('2d');
  if (!context) return null;
  context.scale(ratio, ratio);
  context.fillStyle = document.background; context.fillRect(0, 0, canvas.width / ratio, canvas.height / ratio);
  context.translate(padding - bounds.x, padding - bounds.y);
  if (paper) paintPaper(context, paper, bounds, ratio);
  drawElementsOnPages(context, document.elements, paper);
  return new Promise(resolve => canvas.toBlob(resolve, 'image/png'));
}

/**
 * A notebook's page as a PNG at `scale` pixels per point: its sheet, template and the elements that belong to it
 * (`elementPage`), clipped to the page as on screen; ink from the next page stays there.
 */
export async function renderPagePng(document: CanvasDocument, pageId: string, scale = 2): Promise<Blob | null> {
  const paper = document.paper, rect = paper && pageRects(paper).find(page => page.id === pageId);
  if (!paper || !rect) return null;
  const elements = document.elements.filter(element => elementPage(paper, element).id === rect.id);
  await loadCanvasImages(elements);
  const ratio = Math.min(scale, 4096 / Math.max(rect.width, rect.height));
  const canvas = window.document.createElement('canvas');
  canvas.width = Math.ceil(rect.width * ratio);
  canvas.height = Math.ceil(rect.height * ratio);
  const context = canvas.getContext('2d');
  if (!context) return null;
  context.scale(ratio, ratio);
  context.translate(-rect.x, -rect.y);
  context.fillStyle = PAGE_COLOR; context.fillRect(rect.x, rect.y, rect.width, rect.height);
  paintTemplate(context, rect, ratio);
  clipToPage(context, rect);
  for (const element of elements) drawElement(context, element);
  return new Promise(resolve => canvas.toBlob(resolve, 'image/png'));
}

export function selectedBounds(document: CanvasDocument, selected: Set<string>) {
  return selectionBounds(document.elements.filter(element => selected.has(element.id)));
}

export { elementBounds };
