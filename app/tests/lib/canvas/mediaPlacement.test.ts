import { describe, expect, test } from 'bun:test';
import { placeCanvasImages } from '../../../src/lib/canvas/mediaPlacement';

const source = (name: string, width = 1200, height = 800) => ({ name, width, height, dataUrl: 'data:image/png;base64,fixture' });
describe('canvas image placement', () => {
  test('centres a scaled image in the current viewport and preserves its aspect', () => {
    const [image] = placeCanvasImages([source('photo')], { x: 100, y: -40, zoom: 2 }, { width: 1000, height: 800 });
    expect(image.width / image.height).toBeCloseTo(1.5);
    expect(image.x + image.width / 2).toBe(200);
    expect(image.y + image.height / 2).toBe(220);
    expect(image.width).toBeLessThanOrEqual(480);
    expect(image.naturalWidth).toBe(1200);
    expect(image.sourceName).toBe('photo');
  });
  test('stacks PDF pages vertically in reading order with the first page in view', () => {
    const images = placeCanvasImages(Array.from({ length: 5 }, (_, i) => source(`Page ${i + 1}`, 600, 800)), { x: 0, y: 0, zoom: 1 }, { width: 1000, height: 800 });
    expect(new Set(images.map(image => image.id)).size).toBe(5);
    expect(images[0].y + images[0].height / 2).toBe(400);
    for (let i = 1; i < images.length; i++) {
      expect(images[i].x).toBe(images[0].x);
      expect(images[i].y).toBe(images[i - 1].y + images[i - 1].height + 24);
    }
    expect(images.map(image => image.sourceName)).toEqual(['Page 1', 'Page 2', 'Page 3', 'Page 4', 'Page 5']);
  });
  test('mixed page sizes keep their proportions and share a centre line', () => {
    const images = placeCanvasImages([source('Portrait', 600, 800), source('Landscape', 800, 600)], { x: 200, y: -100, zoom: 2 }, { width: 1000, height: 800 });
    expect(images[0].width / images[0].height).toBeCloseTo(0.75);
    expect(images[1].width / images[1].height).toBeCloseTo(4 / 3);
    expect(images[0].x + images[0].width / 2).toBe(150);
    expect(images[1].x + images[1].width / 2).toBe(150);
    expect(images[1].y).toBe(images[0].y + images[0].height + 24);
  });
  test('empty imports do nothing and invalid dimensions fail before insertion', () => {
    expect(placeCanvasImages([], { x: 0, y: 0, zoom: 1 }, { width: 0, height: 0 })).toEqual([]);
    expect(() => placeCanvasImages([source('bad', 0)], { x: 0, y: 0, zoom: 1 }, { width: 800, height: 600 })).toThrow('dimensions');
  });
});
