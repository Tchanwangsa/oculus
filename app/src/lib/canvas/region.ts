import { elementBounds, type Bounds } from './geometry';
import type { CanvasDocument, CanvasElement, Point } from './model';
import { drawElement, resizeHandles } from './render';
import { paintPaper } from './renderPaper';
import { visibleElements } from './engine';
import { loadCanvasImages } from './renderImages';

export interface CanvasRegion {
  kind: 'capture' | 'crop';
  bounds: Bounds;
  snapshot: CanvasDocument;
  source?: CanvasElement;
}

export function regionBounds(start: Point, end: Point, source?: CanvasElement): Bounds {
  const box = source ? elementBounds(source) : null;
  const clamp = (point: Point): Point => box ? {
    x: Math.max(box.x, Math.min(box.x + box.width, point.x)),
    y: Math.max(box.y, Math.min(box.y + box.height, point.y)),
  } : point;
  const a = clamp(start), b = clamp(end);
  return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y) };
}

/** Crops preserve source pixels and transparency; captures flatten the paper and all intersecting layers. */
export async function rasterizeRegion(region: CanvasRegion): Promise<{ blob: Blob; width: number; height: number }> {
  const { bounds, snapshot, source } = region;
  if (bounds.width <= 0 || bounds.height <= 0) throw new Error('Drag an area with a width and height.');
  const elements = source ? [{ ...source, opacity: 1 }] : visibleElements(snapshot.elements, bounds);
  await Promise.all([loadCanvasImages(elements), window.document.fonts?.ready]);
  const desired = source ? Math.max((source.naturalWidth ?? Math.abs(source.width)) / Math.abs(source.width), (source.naturalHeight ?? Math.abs(source.height)) / Math.abs(source.height)) : 2;
  const scale = Math.min(desired, 8192 / Math.max(bounds.width, bounds.height), Math.sqrt(32_000_000 / (bounds.width * bounds.height)));
  const canvas = window.document.createElement('canvas');
  canvas.width = Math.max(1, Math.floor(bounds.width * scale));
  canvas.height = Math.max(1, Math.floor(bounds.height * scale));
  const context = canvas.getContext('2d');
  if (!context) throw new Error('Could not create this image.');
  context.scale(canvas.width / bounds.width, canvas.height / bounds.height);
  context.translate(-bounds.x, -bounds.y);
  if (!source) {
    context.fillStyle = snapshot.background;
    context.fillRect(bounds.x, bounds.y, bounds.width, bounds.height);
    if (snapshot.paper) paintPaper(context, snapshot.paper, bounds, scale);
  }
  for (const element of elements) drawElement(context, element);
  const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, 'image/png'));
  if (!blob) throw new Error('Could not encode this image.');
  return { blob, width: canvas.width, height: canvas.height };
}

/** Supply the PNG promise during the click so Safari keeps clipboard user activation while pixels decode. */
export function copyRegionImage(region: CanvasRegion): Promise<void> {
  if (!navigator.clipboard?.write || typeof ClipboardItem === 'undefined') throw new Error('Image copying is unavailable here. Use a browser with image clipboard support to copy this capture.');
  return navigator.clipboard.write([new ClipboardItem({ 'image/png': rasterizeRegion(region).then(result => result.blob) })]);
}

/** Corners win over edges; the whole edge is grabbable, with extra room for a Pencil or finger. */
export function regionHandleAt(bounds: Bounds, point: Point, zoom: number, pointerType = 'mouse'): number | undefined {
  const tolerance = (pointerType === 'mouse' ? 9 : 16) / zoom;
  let corner: number | undefined, distance = tolerance;
  for (const [index, handle] of resizeHandles(bounds).slice(0, 4).entries()) {
    const next = Math.hypot(handle.x - point.x, handle.y - point.y);
    if (next <= distance) { corner = index; distance = next; }
  }
  if (corner !== undefined) return corner;
  const candidates: { handle: number; distance: number }[] = [];
  if (point.x >= bounds.x - tolerance && point.x <= bounds.x + bounds.width + tolerance) {
    candidates.push({ handle: 4, distance: Math.abs(point.y - bounds.y) }, { handle: 6, distance: Math.abs(point.y - bounds.y - bounds.height) });
  }
  if (point.y >= bounds.y - tolerance && point.y <= bounds.y + bounds.height + tolerance) {
    candidates.push({ handle: 5, distance: Math.abs(point.x - bounds.x - bounds.width) }, { handle: 7, distance: Math.abs(point.x - bounds.x) });
  }
  return candidates.filter(candidate => candidate.distance <= tolerance).sort((a, b) => a.distance - b.distance)[0]?.handle;
}

export function resizeRegion(bounds: Bounds, handle: number, point: Point, source: CanvasElement | undefined, zoom: number): Bounds {
  const full = source ? elementBounds(source) : undefined;
  const minWidth = Math.min(full?.width ?? Infinity, bounds.width, 8 / zoom), minHeight = Math.min(full?.height ?? Infinity, bounds.height, 8 / zoom);
  const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
  let left = bounds.x, top = bounds.y, right = left + bounds.width, bottom = top + bounds.height;
  if ([0, 3, 7].includes(handle)) left = clamp(point.x, full?.x ?? -Infinity, right - minWidth);
  if ([1, 2, 5].includes(handle)) right = clamp(point.x, left + minWidth, full ? full.x + full.width : Infinity);
  if ([0, 1, 4].includes(handle)) top = clamp(point.y, full?.y ?? -Infinity, bottom - minHeight);
  if ([2, 3, 6].includes(handle)) bottom = clamp(point.y, top + minHeight, full ? full.y + full.height : Infinity);
  return { x: left, y: top, width: right - left, height: bottom - top };
}

export function moveRegion(bounds: Bounds, delta: Point, source?: CanvasElement): Bounds {
  if (!source) return { ...bounds, x: bounds.x + delta.x, y: bounds.y + delta.y };
  const full = elementBounds(source);
  return { ...bounds, x: Math.max(full.x, Math.min(full.x + full.width - bounds.width, bounds.x + delta.x)), y: Math.max(full.y, Math.min(full.y + full.height - bounds.height, bounds.y + delta.y)) };
}
