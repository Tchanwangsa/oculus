import type { CanvasElement, CanvasViewport } from './model';
import { secureId } from './ids';

export interface CanvasImageSource { dataUrl: string; width: number; height: number; name: string }

/** Stack a batch in reading order, with its first page centred in the visible view. */
export function placeCanvasImages(images: readonly CanvasImageSource[], viewport: CanvasViewport, size: { width: number; height: number }): CanvasElement[] {
  if (!images.length) return [];
  const gap = 24;
  const maxWidth = Math.min(480, Math.max(160, (size.width - 96) / viewport.zoom));
  const maxHeight = Math.min(600, Math.max(160, (size.height - 180) / viewport.zoom));
  const boxes = images.map(image => {
    if (!Number.isFinite(image.width) || !Number.isFinite(image.height) || image.width <= 0 || image.height <= 0) throw new Error('An imported image has invalid dimensions.');
    const scale = Math.min(1, maxWidth / image.width, maxHeight / image.height);
    return { width: image.width * scale, height: image.height * scale };
  });
  const centre = (size.width / 2 - viewport.x) / viewport.zoom;
  let top = (size.height / 2 - viewport.y) / viewport.zoom - boxes[0].height / 2;
  return images.map((image, index) => {
    const box = boxes[index];
    const y = top;
    top += box.height + gap;
    return {
      id: secureId(), type: 'image',
      x: centre - box.width / 2,
      y,
      ...box, dataUrl: image.dataUrl, sourceName: image.name,
      naturalWidth: image.width, naturalHeight: image.height,
      stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0,
    };
  });
}
