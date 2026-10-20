import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, parseCanvasDocument, serializeCanvasDocument, type CanvasElement } from '../../../src/lib/canvas/model';
import { isCanvasImageDataUrl } from '../../../src/lib/canvas/imageSource';
import { elementBounds, elementHitTest } from '../../../src/lib/canvas/geometry';
import { cloneCanvasElements, parseCanvasClipboard, serializeCanvasClipboard } from '../../../src/lib/canvas/renderClipboard';
import { preserveImageAspectRatio, resizeElements } from '../../../src/lib/canvas/resize';
import { exportCanvasSvg } from '../../../src/lib/canvas/renderSvg';
import { inspectorSections, inspectorTarget, patchElementStyle } from '../../../src/lib/canvas/style';
import { drawElement, exportCanvasPng } from '../../../src/lib/canvas/render';
import { loadCanvasImages, subscribeCanvasImages } from '../../../src/lib/canvas/renderImages';

const png = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=';
const image: CanvasElement = {
  id: 'image', type: 'image', x: -100, y: 40, width: 200, height: 100, stroke: '#000000', fill: 'transparent',
  strokeWidth: 0, style: 'clean', seed: 1, dataUrl: png, sourceName: 'Lecture page 3', naturalWidth: 1200, naturalHeight: 600,
};
const document = (elements: CanvasElement[]) => ({ ...createCanvasDocument(), elements });

describe('portable canvas images', () => {
  test('roundtrips embedded pixels, source metadata and extension fields through files and clipboard', () => {
    const element = { ...image, opacity: 0.4, futureMetadata: { page: 3 } };
    const saved = serializeCanvasDocument(document([element]));
    expect(parseCanvasDocument(saved).elements).toEqual([element]);
    expect(parseCanvasClipboard(serializeCanvasClipboard([element]))).toEqual([element]);
    const cloned = cloneCanvasElements([element], { x: 30, y: 20 }, () => 'copy')[0];
    expect(cloned).toMatchObject({ id: 'copy', x: -70, y: 60, dataUrl: png, sourceName: 'Lecture page 3', naturalWidth: 1200, naturalHeight: 600, opacity: 0.4 });
    expect(element.x).toBe(-100);
  });

  test('allows supported raster signatures and refuses remote, active or malformed image sources', () => {
    const data = (mime: string, bytes: string) => `data:image/${mime};base64,${btoa(bytes)}`;
    expect(isCanvasImageDataUrl(png)).toBe(true);
    expect(isCanvasImageDataUrl(data('jpeg', '\xff\xd8\xff\xe0\x00\x10JFIF\x00\x00'))).toBe(true);
    expect(isCanvasImageDataUrl(data('gif', 'GIF89a\x01\x00\x01\x00\x00\x00'))).toBe(true);
    expect(isCanvasImageDataUrl(data('webp', 'RIFF\x00\x00\x00\x00WEBP'))).toBe(true);
    for (const source of [undefined, '', 'https://example.com/image.png', 'file:///image.png', 'data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=', data('png', '<svg></svg>!'), `${png} garbage`, png.slice(0, -1)]) {
      expect(isCanvasImageDataUrl(source)).toBe(false);
      expect(() => serializeCanvasDocument(document([{ ...image, dataUrl: source }]))).toThrow('embedded');
    }
    for (const patch of [{ sourceName: 9 }, { naturalWidth: 0 }, { naturalHeight: -1 }, { naturalWidth: Infinity }]) {
      expect(() => serializeCanvasDocument(document([{ ...image, ...patch } as CanvasElement]))).toThrow();
    }
  });

  test('selects the full image and resizes without altering embedded pixels or intrinsic dimensions', () => {
    expect(elementHitTest(image, { x: 0, y: 90 }, 0)).toBe(true);
    expect(elementHitTest(image, { x: 101, y: 90 }, 0)).toBe(false);
    const from = elementBounds(image), to = { x: -200, y: -20, width: 400, height: 200 };
    const resized = resizeElements([image], new Set([image.id]), from, to).get(image.id)!;
    expect(resized).toMatchObject({ ...to, dataUrl: png, naturalWidth: 1200, naturalHeight: 600 });
    const reversed = { ...image, width: -200, height: -100 };
    expect(elementBounds(reversed)).toEqual({ x: -300, y: -60, width: 200, height: 100 });
    expect(elementHitTest(reversed, { x: -200, y: -10 }, 0)).toBe(true);
  });

  test('offers opacity and layer editing without shape or text appearance controls', () => {
    expect(inspectorSections([inspectorTarget(image)])).toEqual(['opacity']);
    expect(patchElementStyle(image, { opacity: 0.3, fill: '#ff0000', strokeWidth: 10 })).toEqual({ ...image, opacity: 0.3 });
  });

  test('corner resizing preserves a single image aspect ratio in every drag direction, with Shift to stretch', () => {
    const from = { x: 0, y: 0, width: 200, height: 100 }, ids = new Set([image.id]);
    expect(preserveImageAspectRatio([image], ids, from, { x: 300, y: 200 }, { x: 0, y: 0 })).toEqual({ x: 0, y: 0, width: 400, height: 200 });
    expect(preserveImageAspectRatio([image], ids, from, { x: -100, y: -200 }, { x: 200, y: 100 })).toEqual({ x: -400, y: -200, width: 600, height: 300 });
    expect(preserveImageAspectRatio([image], ids, from, { x: 100, y: -100 }, { x: 0, y: 100 })).toEqual({ x: 0, y: -100, width: 400, height: 200 });
    const stretched = { x: 0, y: 0, width: 300, height: 200 };
    expect(preserveImageAspectRatio([image], ids, from, { x: 300, y: 200 }, { x: 0, y: 0 }, true)).toEqual(stretched);
    expect(preserveImageAspectRatio([{ ...image, type: 'rectangle' }], ids, from, { x: 300, y: 200 }, { x: 0, y: 0 })).toEqual(stretched);
  });

  test('SVG embeds raster pixels with opacity and normalized dimensions in layer order', () => {
    const reversed = { ...image, width: -200, height: -100, opacity: 0.5 };
    const exported = exportCanvasSvg(document([reversed, { ...image, id: 'front', x: 500 }]));
    expect(exported).toContain(`opacity="0.5"><image x="-200" y="-100" width="200" height="100" href="${png}" preserveAspectRatio="none"/>`);
    expect(exported.match(/<image /g)).toHaveLength(2);
    expect(exported.indexOf('translate(-100,40)')).toBeLessThan(exported.indexOf('translate(500,40)'));
    expect(() => exportCanvasSvg(document([{ ...image, dataUrl: 'https://example.com/image.png' }]))).toThrow('embedded raster');
  });

  test('repaints after decode and PNG export waits for pixels, sharing decoded data across edits', async () => {
    const oldImage = globalThis.Image, oldWindow = globalThis.window;
    const pending: { onload: (() => void) | null; onerror: (() => void) | null }[] = [];
    let decodedDraws = 0, blobs = 0, repaints = 0;
    class TestImage {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      set src(_source: string) { pending.push(this); }
    }
    const context = {
      save() {}, restore() {}, translate() {}, scale() {}, fillRect() {}, drawImage() { decodedDraws++; },
    } as unknown as CanvasRenderingContext2D;
    const canvas = { width: 0, height: 0, getContext: () => context, toBlob: (callback: (blob: Blob) => void) => { blobs++; callback(new Blob(['pixels'], { type: 'image/png' })); } };
    Object.assign(globalThis, { Image: TestImage, window: { document: { createElement: () => canvas } } });
    const unsubscribe = subscribeCanvasImages(() => { repaints++; });
    try {
      const element = { ...image, sourceName: 'Awaited image' };
      drawElement(context, element);
      const result = exportCanvasPng(document([element]));
      await Promise.resolve();
      expect(blobs).toBe(0);
      expect(pending).toHaveLength(1);
      pending[0].onload!();
      expect(await result).toBeInstanceOf(Blob);
      expect(repaints).toBe(1);
      expect(decodedDraws).toBe(1);
      await loadCanvasImages([{ ...element, id: 'clone', x: 50 }]);
      expect(pending).toHaveLength(1);
      drawElement(context, { ...element, width: 100 });
      expect(decodedDraws).toBe(2);
    } finally {
      unsubscribe();
      Object.assign(globalThis, { Image: oldImage, window: oldWindow });
    }
  });
});
