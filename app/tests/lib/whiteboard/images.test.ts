import { describe, expect, test } from 'bun:test';
import { createWhiteboardDocument, parseWhiteboardDocument, serializeWhiteboardDocument, type WhiteboardElement } from '../../../src/lib/whiteboard/model';
import { isWhiteboardImageDataUrl } from '../../../src/lib/whiteboard/imageSource';
import { elementBounds, elementHitTest } from '../../../src/lib/whiteboard/geometry';
import { cloneWhiteboardElements, parseWhiteboardClipboard, serializeWhiteboardClipboard } from '../../../src/lib/whiteboard/renderClipboard';
import { resizeElements } from '../../../src/lib/whiteboard/resize';
import { exportWhiteboardSvg } from '../../../src/lib/whiteboard/renderSvg';
import { inspectorSections, inspectorTarget, patchElementStyle } from '../../../src/lib/whiteboard/style';
import { drawElement, exportWhiteboardPng } from '../../../src/lib/whiteboard/render';
import { loadWhiteboardImages, subscribeWhiteboardImages } from '../../../src/lib/whiteboard/renderImages';

const png = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=';
const image: WhiteboardElement = {
  id: 'image', type: 'image', x: -100, y: 40, width: 200, height: 100, stroke: '#000000', fill: 'transparent',
  strokeWidth: 0, style: 'clean', seed: 1, dataUrl: png, sourceName: 'Lecture page 3', naturalWidth: 1200, naturalHeight: 600,
};
const document = (elements: WhiteboardElement[]) => ({ ...createWhiteboardDocument(), elements });

describe('portable whiteboard images', () => {
  test('roundtrips embedded pixels, source metadata and extension fields through files and clipboard', () => {
    const element = { ...image, opacity: 0.4, futureMetadata: { page: 3 } };
    const saved = serializeWhiteboardDocument(document([element]));
    expect(parseWhiteboardDocument(saved).elements).toEqual([element]);
    expect(parseWhiteboardClipboard(serializeWhiteboardClipboard([element]))).toEqual([element]);
    const cloned = cloneWhiteboardElements([element], { x: 30, y: 20 }, () => 'copy')[0];
    expect(cloned).toMatchObject({ id: 'copy', x: -70, y: 60, dataUrl: png, sourceName: 'Lecture page 3', naturalWidth: 1200, naturalHeight: 600, opacity: 0.4 });
    expect(element.x).toBe(-100);
  });

  test('allows supported raster signatures and refuses remote, active or malformed image sources', () => {
    const data = (mime: string, bytes: string) => `data:image/${mime};base64,${btoa(bytes)}`;
    expect(isWhiteboardImageDataUrl(png)).toBe(true);
    expect(isWhiteboardImageDataUrl(data('jpeg', '\xff\xd8\xff\xe0\x00\x10JFIF\x00\x00'))).toBe(true);
    expect(isWhiteboardImageDataUrl(data('gif', 'GIF89a\x01\x00\x01\x00\x00\x00'))).toBe(true);
    expect(isWhiteboardImageDataUrl(data('webp', 'RIFF\x00\x00\x00\x00WEBP'))).toBe(true);
    for (const source of [undefined, '', 'https://example.com/image.png', 'file:///image.png', 'data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=', data('png', '<svg></svg>!'), `${png} garbage`, png.slice(0, -1)]) {
      expect(isWhiteboardImageDataUrl(source)).toBe(false);
      expect(() => serializeWhiteboardDocument(document([{ ...image, dataUrl: source }]))).toThrow('embedded');
    }
    for (const patch of [{ sourceName: 9 }, { naturalWidth: 0 }, { naturalHeight: -1 }, { naturalWidth: Infinity }]) {
      expect(() => serializeWhiteboardDocument(document([{ ...image, ...patch } as WhiteboardElement]))).toThrow();
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

  test('SVG embeds raster pixels with opacity and normalized dimensions in layer order', () => {
    const reversed = { ...image, width: -200, height: -100, opacity: 0.5 };
    const exported = exportWhiteboardSvg(document([reversed, { ...image, id: 'front', x: 500 }]));
    expect(exported).toContain(`opacity="0.5"><image x="-200" y="-100" width="200" height="100" href="${png}" preserveAspectRatio="none"/>`);
    expect(exported.match(/<image /g)).toHaveLength(2);
    expect(exported.indexOf('translate(-100,40)')).toBeLessThan(exported.indexOf('translate(500,40)'));
    expect(() => exportWhiteboardSvg(document([{ ...image, dataUrl: 'https://example.com/image.png' }]))).toThrow('embedded raster');
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
    const unsubscribe = subscribeWhiteboardImages(() => { repaints++; });
    try {
      const element = { ...image, sourceName: 'Awaited image' };
      drawElement(context, element);
      const result = exportWhiteboardPng(document([element]));
      await Promise.resolve();
      expect(blobs).toBe(0);
      expect(pending).toHaveLength(1);
      pending[0].onload!();
      expect(await result).toBeInstanceOf(Blob);
      expect(repaints).toBe(1);
      expect(decodedDraws).toBe(1);
      await loadWhiteboardImages([{ ...element, id: 'clone', x: 50 }]);
      expect(pending).toHaveLength(1);
      drawElement(context, { ...element, width: 100 });
      expect(decodedDraws).toBe(2);
    } finally {
      unsubscribe();
      Object.assign(globalThis, { Image: oldImage, window: oldWindow });
    }
  });
});
