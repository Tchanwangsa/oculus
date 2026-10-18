import type { CanvasElement } from './model';
import type { Bounds } from './geometry';
import type { ElementChanges } from './changes';
import { growToFitLabel } from './labels';
import { scaleFixedSegments } from './elbow/lanes';

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
