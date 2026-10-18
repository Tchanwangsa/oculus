import { LABELLED_TYPES, hasLabel, type CanvasElement, type Point } from './model';
import { elementBounds, elementHitTest } from './geometry';
import { LABEL_PADDING, labelBox, layoutShapeLabel, measureCanvasText, measureTextElement, type TextMeasure } from './renderText';
import { patchElementStyle, type StylePatch } from './style';
import type { ElementChanges } from './changes';

const LABEL_SCALE: Partial<Record<CanvasElement['type'], number>> = { ellipse: Math.SQRT1_2, diamond: 0.5 };

/** A label taller than its box grows the shape downward (top edge fixed); a shape never shrinks to its label. */
export function growToFitLabel(el: CanvasElement, measure: TextMeasure = measureCanvasText): CanvasElement {
  if (!hasLabel(el)) return el;
  const needed = layoutShapeLabel(el, measure).height;
  if (needed <= labelBox(el).height + 1e-6) return el;
  const height = (needed + LABEL_PADDING * 2) / (LABEL_SCALE[el.type] ?? 1);
  return { ...el, y: Math.min(el.y, el.y + el.height), height };
}

/** Empty text removes the label, never the shape. */
export function commitShapeLabel(shape: CanvasElement, text: string, measure: TextMeasure = measureCanvasText): CanvasElement {
  if (!text.trim()) {
    if (shape.text === undefined) return shape;
    const { text: _removed, ...rest } = shape;
    return rest as CanvasElement;
  }
  return growToFitLabel(shape.text === text ? shape : { ...shape, text }, measure);
}

/** What a finished edit writes. An empty label leaves the shape; empty standalone text is removed (null). */
export function commitTextEdit(base: CanvasElement, kind: 'text' | 'label', text: string, measure: TextMeasure = measureCanvasText): CanvasElement | null {
  if (kind === 'label') return commitShapeLabel(base, text, measure);
  return text.trim() ? measureTextElement({ ...base, text }, measure) : null;
}

/** Whether a point is inside a shape's outline, hollow or not. */
export function pointInShape(el: CanvasElement, p: Point): boolean {
  const b = elementBounds(el);
  if (!b.width || !b.height) return false;
  const dx = Math.abs(p.x - b.x - b.width / 2) / (b.width / 2), dy = Math.abs(p.y - b.y - b.height / 2) / (b.height / 2);
  if (el.type === 'ellipse') return dx * dx + dy * dy <= 1;
  if (el.type === 'diamond') return dx + dy <= 1;
  return dx <= 1 && dy <= 1;
}

/** The topmost shape that can take a label at a point, including the inside of hollow shapes, which hit testing skips. */
export function labelTargetAt(elements: readonly CanvasElement[], p: Point, tolerance = 6): CanvasElement | undefined {
  for (let i = elements.length - 1; i >= 0; i--) {
    const el = elements[i];
    if (LABELLED_TYPES.has(el.type) && (pointInShape(el, p) || elementHitTest(el, p, tolerance))) return el;
  }
  return undefined;
}

/** A font change re-measures standalone text and re-runs the grow rule on labelled shapes. */
export function restyleElement(el: CanvasElement, patch: StylePatch, measure: TextMeasure = measureCanvasText): CanvasElement {
  const next = patchElementStyle(el, patch);
  if (next === el || (next.fontSize === el.fontSize && next.fontFamily === el.fontFamily)) return next;
  if (next.type === 'text') return measureTextElement(next, measure);
  return growToFitLabel(next, measure);
}

/** Only elements the patch actually changes, so an inapplicable patch commits nothing. */
export function stylePatchChanges(elements: readonly CanvasElement[], ids: ReadonlySet<string>, patch: StylePatch, measure: TextMeasure = measureCanvasText): ElementChanges {
  const changes = new Map<string, CanvasElement>();
  for (const el of elements) {
    if (!ids.has(el.id)) continue;
    const next = restyleElement(el, patch, measure);
    if (next !== el) changes.set(el.id, next);
  }
  return changes;
}
