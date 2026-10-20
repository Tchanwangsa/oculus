import type { CanvasElement, Point } from './model';
import type { Bounds } from './geometry';
import type { ElementChanges } from './changes';
import { growToFitLabel } from './labels';
import { scaleFixedSegments } from './elbow/lanes';

/** A single image keeps its displayed proportions around the fixed opposite corner; Shift allows stretching. */
export function preserveImageAspectRatio(elements: readonly CanvasElement[], ids: ReadonlySet<string>, from: Bounds, point: Point, opposite: Point, free = false): Bounds {
  let dx = point.x - opposite.x, dy = point.y - opposite.y;
  const image = ids.size === 1 ? elements.find(element => ids.has(element.id)) : undefined;
  if (!free && image?.type === 'image' && from.width > 0 && from.height > 0) {
    const ratio = from.width / from.height;
    const width = Math.max(Math.abs(dx), Math.abs(dy) * ratio);
    dx = width * (Math.sign(dx) || (opposite.x <= from.x ? 1 : -1));
    dy = width / ratio * (Math.sign(dy) || (opposite.y <= from.y ? 1 : -1));
  }
  return { x: Math.min(opposite.x, opposite.x + dx), y: Math.min(opposite.y, opposite.y + dy), width: Math.abs(dx), height: Math.abs(dy) };
}

/**
 * Scales the selected elements from `from` to `to`. Every point of a line,
 * arrow or stroke scales, so bends and curves keep their shape (a curve is
 * affine in its points). Text scales its font; a shape keeps its label's size,
 * rewraps it and grows if the label no longer fits.
 */
export function resizeElements(elements: readonly CanvasElement[], ids: ReadonlySet<string>, from: Bounds, to: Bounds): ElementChanges {
  const sx = to.width / Math.max(from.width, 1), sy = to.height / Math.max(from.height, 1);
  const changes = new Map<string, CanvasElement>();
  for (const element of elements) {
    if (!ids.has(element.id)) continue;
    const resized: CanvasElement = {
      ...element, x: to.x + (element.x - from.x) * sx, y: to.y + (element.y - from.y) * sy,
      width: element.width * (element.type === 'text' ? sy : sx), height: element.height * sy,
      points: element.points?.map(p => ({ x: p.x * sx, y: p.y * sy })),
      ...(element.fixedSegments ? { fixedSegments: scaleFixedSegments(element.fixedSegments, sx, sy) } : {}),
      ...(element.type === 'text' ? { fontSize: Math.max(8, (element.fontSize ?? 24) * sy) } : {}),
    };
    changes.set(element.id, growToFitLabel(resized));
  }
  return changes;
}
