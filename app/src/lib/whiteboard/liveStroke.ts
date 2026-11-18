import { type CanvasElement, type CanvasViewport, type Point } from './model';
import type { Paper, PageRect } from './paper';
import { clipToPage, elementPage } from './renderPaper';

/**
 * The freehand stroke being drawn, painted on two canvases above the scene so a pen sample never repaints the scene.
 * A pen that lays one solid round line is painted piece by piece: `ink` only gains the curve pieces that later
 * samples cannot change, and `tail` holds the one straight piece from the last settled point to the pen, erased and
 * redrawn on each sample; together they trace the path `elementPaths` in `render.ts` builds, at full alpha, and the
 * host fades the layer that holds them. Every other pen is painted whole on `tail` each sample by the `paint` the
 * host passes, which is the scene's own drawing of the element.
 */
export type LiveStrokeContext = Pick<CanvasRenderingContext2D,
  'setTransform' | 'clearRect' | 'beginPath' | 'moveTo' | 'lineTo' | 'quadraticCurveTo' | 'stroke' | 'strokeStyle' | 'lineWidth' | 'lineCap' | 'lineJoin' | 'save' | 'restore' | 'rect' | 'clip'
> & { canvas: { width: number; height: number } };
export type LiveStrokePaint = (context: CanvasRenderingContext2D, element: CanvasElement) => void;

interface Placement { element: CanvasElement; scale: number; dx: number; dy: number; paint?: LiveStrokePaint }
type Box = [x: number, y: number, width: number, height: number];
const mid = (a: Point, b: Point) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
/** A nib pen's ink reaches this many stroke widths from its path at most. */
const REACH = 2;

export class LiveStroke {
  private placement: Placement | null = null;
  /** The next curve piece to settle: piece `i` bends at point `i` and is final once point `i + 1` exists. */
  private settled = 1;
  /** What the tail canvas holds, in canvas pixels. */
  private tailBox: Box | null = null;
  /** Running bounds for the notebook page owner and the whole-painted tail's dirty rectangle. */
  private seen = 0;
  private bounds = { left: 0, top: 0, right: 0, bottom: 0 };
  private painted = false;
  private page?: PageRect;

  constructor(private ink: LiveStrokeContext, private tail: LiveStrokeContext) {}

  /** `element` gives the stroke's origin and look. With `paint` the stroke is painted whole; without, piece by piece. */
  begin(element: CanvasElement, viewport: CanvasViewport, pixelRatio: number, paint?: LiveStrokePaint) {
    this.clear();
    const scale = pixelRatio * viewport.zoom;
    this.placement = paint
      ? { element, scale, dx: pixelRatio * viewport.x, dy: pixelRatio * viewport.y, paint }
      : { element, scale, dx: pixelRatio * (viewport.x + viewport.zoom * element.x), dy: pixelRatio * (viewport.y + viewport.zoom * element.y) };
    this.prepare();
  }

  private prepare() {
    const placement = this.placement;
    if (!placement) return;
    for (const context of [this.ink, this.tail]) {
      context.setTransform(placement.scale, 0, 0, placement.scale, placement.dx, placement.dy);
      if (placement.paint) continue;
      context.strokeStyle = placement.element.stroke;
      context.lineWidth = placement.element.strokeWidth;
      context.lineCap = 'round';
      context.lineJoin = 'round';
    }
  }

  /** Paints what `element.points` (element-local, only ever appended to) has gained since the last call. */
  draw(element: CanvasElement, paper?: Paper) {
    const placement = this.placement, points = element.points;
    if (!placement || !points || points.length < 2) return;
    if (paper || placement.paint) this.extendBounds(points);
    const { left, top, right, bottom } = this.bounds;
    const page = paper && elementPage(paper, element, { x: element.x + left, y: element.y + top, width: right - left, height: bottom - top });
    if (page !== this.page) {
      // The centre of a growing stroke can cross onto another page. Replay the settled ink inside its new clip.
      this.clearPixels();
      this.prepare();
      this.page = page;
    }
    this.eraseTail();
    if (page) {
      for (const context of [this.ink, this.tail]) {
        context.save();
        clipToPage(context, page, placement.paint ? undefined : element);
      }
    }
    if (placement.paint) this.drawWhole(placement, element);
    else this.drawPieces(placement, points);
    if (page) { this.ink.restore(); this.tail.restore(); }
    this.painted = true;
  }

  private drawPieces(placement: Placement, points: readonly Point[]) {
    const { ink, tail } = this, last = points.length - 1;
    if (this.settled < last) {
      const from = this.settled === 1 ? points[0] : mid(points[this.settled - 1], points[this.settled]);
      ink.beginPath();
      ink.moveTo(from.x, from.y);
      for (; this.settled < last; this.settled++) {
        const bend = points[this.settled], to = mid(bend, points[this.settled + 1]);
        ink.quadraticCurveTo(bend.x, bend.y, to.x, to.y);
      }
      ink.stroke();
    }
    const from = last > 1 ? mid(points[last - 1], points[last]) : points[0], to = points[last];
    tail.beginPath(); tail.moveTo(from.x, from.y); tail.lineTo(to.x, to.y); tail.stroke();
    const { scale, dx, dy } = placement, pad = placement.element.strokeWidth * scale / 2 + 2;
    const x = Math.min(from.x, to.x) * scale + dx - pad, y = Math.min(from.y, to.y) * scale + dy - pad;
    this.tailBox = [x, y, Math.abs(to.x - from.x) * scale + pad * 2, Math.abs(to.y - from.y) * scale + pad * 2];
  }

  private extendBounds(points: readonly Point[]) {
    const bounds = this.bounds;
    for (; this.seen < points.length; this.seen++) {
      const { x, y } = points[this.seen];
      if (!this.seen) { bounds.left = bounds.right = x; bounds.top = bounds.bottom = y; continue; }
      bounds.left = Math.min(bounds.left, x); bounds.right = Math.max(bounds.right, x);
      bounds.top = Math.min(bounds.top, y); bounds.bottom = Math.max(bounds.bottom, y);
    }
  }

  private drawWhole(placement: Placement, element: CanvasElement) {
    const bounds = this.bounds;
    placement.paint!(this.tail as CanvasRenderingContext2D, element);
    const { scale, dx, dy } = placement, pad = Math.max(element.strokeWidth * REACH, 2) * scale + 4;
    this.tailBox = [
      (element.x + bounds.left) * scale + dx - pad, (element.y + bounds.top) * scale + dy - pad,
      (bounds.right - bounds.left) * scale + pad * 2, (bounds.bottom - bounds.top) * scale + pad * 2,
    ];
  }

  private eraseTail() {
    const placement = this.placement;
    if (!this.tailBox || !placement) return;
    this.tail.setTransform(1, 0, 0, 1, 0, 0);
    this.tail.clearRect(...this.tailBox);
    this.tail.setTransform(placement.scale, 0, 0, placement.scale, placement.dx, placement.dy);
    this.tailBox = null;
  }

  private clearPixels() {
    if (this.painted) for (const context of [this.ink, this.tail]) {
      context.setTransform(1, 0, 0, 1, 0, 0);
      context.clearRect(0, 0, context.canvas.width, context.canvas.height);
    }
    this.settled = 1; this.tailBox = null; this.painted = false;
  }

  /** Ends the stroke and empties both canvases. */
  clear() {
    this.clearPixels();
    this.placement = null; this.seen = 0; this.page = undefined;
  }
}
