import { describe, expect, test } from 'bun:test';
import { placeCanvasImages, placeImagesIn, placeNotebookImages } from '../../../src/lib/canvas/mediaPlacement';
import { createCanvasDocument } from '../../../src/lib/canvas/model';

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

test('a dropped batch centres its first image at the world drop point regardless of pan and zoom', () => {
  const images = placeCanvasImages([source('first'), source('second')], { x: -300, y: 80, zoom: 1.3 }, { width: 900, height: 700 }, { x: 1300, y: -250 });
  expect(images[0].x + images[0].width / 2).toBe(1300);
  expect(images[0].y + images[0].height / 2).toBe(-250);
  expect(images[1].y).toBe(images[0].y + images[0].height + 24);
});

describe('placing images in a notebook', () => {
  // Page one spans x -100..100, y 0..300; page two y 310..610.
  const paper = { gap: 10, pages: [{ id: 'p1', width: 200, height: 300 }, { id: 'p2', width: 200, height: 300 }] };
  const view = { x: 500, y: 100, zoom: 1 }, size = { width: 1000, height: 800 };
  const centre = (image: { x: number; y: number; width: number; height: number }) => ({ x: image.x + image.width / 2, y: image.y + image.height / 2 });
  test('goes onto the page in view, centred on its visible part, and shrinks to fit the page', () => {
    const [image] = placeNotebookImages([source('photo', 1200, 800)], paper, view, size);
    expect(image.width).toBe(200);
    expect(image.height).toBeCloseTo(200 * 800 / 1200);
    expect(centre(image)).toEqual({ x: 0, y: 150 });
  });
  test('a drop onto a page lands there, and one on the desk goes to the page in view, inside its edges', () => {
    const [onTwo] = placeNotebookImages([source('a', 100, 100)], paper, view, size, { x: 20, y: 400 });
    expect(centre(onTwo)).toEqual({ x: 20, y: 400 });
    const [desk] = placeNotebookImages([source('b', 100, 100)], paper, view, size, { x: 400, y: 900 });
    expect(desk.x).toBeGreaterThanOrEqual(-100);
    expect(desk.x + desk.width).toBeLessThanOrEqual(100);
    expect(desk.y + desk.height).toBeLessThanOrEqual(300);
  });
  test('a batch that runs past the page carries on at the top of the next one, and stays on the last', () => {
    const images = placeNotebookImages(Array.from({ length: 4 }, (_, i) => source(`Page ${i}`, 100, 200)), paper, view, size);
    const pages = images.map(image => centre(image).y < 305 ? 1 : 2);
    expect(pages).toEqual([1, 2, 2, 2]);
    for (const image of images) expect(image.y + image.height).toBeLessThanOrEqual(610);
  });
  test('placeImagesIn keeps the plain stack for a canvas, even one with paper', () => {
    const document = { ...createCanvasDocument(), paper, viewport: view };
    const [plain] = placeImagesIn(document, false, [source('wide', 1200, 800)], size);
    expect(plain.width).toBe(480);
    const [bound] = placeImagesIn(document, true, [source('wide', 1200, 800)], size);
    expect(bound.width).toBe(200);
  });
});
