import type { WhiteboardElement, Point } from './model';
import type { Bounds } from './geometry';
import type { ElementChanges } from './changes';
import { growToFitLabel } from './labels';
import { scaleFixedSegments } from './elbow/lanes';

/** Corners scale uniformly; edges stretch one axis, or scale uniformly with Shift.
 * Handle order is NW, NE, SE, SW, N, E, S, W, matching `resizeHandles`.
 */
export function resizeBounds(from: Bounds, handle: number, point: Point, shift = false): Bounds {
  const right = from.x + from.width, bottom = from.y + from.height;
  if (handle < 4) {
    const left = handle === 0 || handle === 3, top = handle < 2;
    const opposite = { x: left ? right : from.x, y: top ? bottom : from.y };
    const dx = point.x - opposite.x, dy = point.y - opposite.y;
    const scale = Math.max(from.width > 0 ? Math.abs(dx) / from.width : 0, from.height > 0 ? Math.abs(dy) / from.height : 0, 0.01);
    const width = from.width * scale, height = from.height * scale;
    return { x: opposite.x + ((Math.sign(dx) || (left ? -1 : 1)) < 0 ? -width : 0), y: opposite.y + ((Math.sign(dy) || (top ? -1 : 1)) < 0 ? -height : 0), width, height };
  }
  const horizontal = handle === 5 || handle === 7, leading = handle === 4 || handle === 7;
  const anchor = horizontal ? (leading ? right : from.x) : (leading ? bottom : from.y);
  const delta = (horizontal ? point.x : point.y) - anchor;
  const extent = Math.max(Math.abs(delta), 1);
  const scale = extent / Math.max(horizontal ? from.width : from.height, 1);
  const width = horizontal ? extent : shift ? from.width * scale : from.width;
  const height = horizontal ? (shift ? from.height * scale : from.height) : extent;
  const start = anchor + ((Math.sign(delta) || (leading ? -1 : 1)) < 0 ? -extent : 0);
  return {
    x: horizontal ? start : from.x + (from.width - width) / 2,
    y: horizontal ? from.y + (from.height - height) / 2 : start,
    width, height,
  };
}

/**
 * Scales the selected elements from `from` to `to`. Every point of a line,
 * arrow or stroke scales, so bends and curves keep their shape (a curve is
 * affine in its points). Text scales its font; a shape keeps its label's size,
 * rewraps it and grows if the label no longer fits.
 */
export function resizeElements(elements: readonly WhiteboardElement[], ids: ReadonlySet<string>, from: Bounds, to: Bounds): ElementChanges {
  const sx = to.width / Math.max(from.width, 1), sy = to.height / Math.max(from.height, 1);
  const changes = new Map<string, WhiteboardElement>();
  for (const element of elements) {
    if (!ids.has(element.id)) continue;
    const resized: WhiteboardElement = {
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
