import { describe, expect, test } from 'bun:test';
import { resizeBounds, resizeElements } from '@/lib/canvas/resize';
import { resizeHandles } from '@/lib/canvas/render';
import type { CanvasElement } from '@/lib/canvas/model';

const from = { x: 100, y: 50, width: 200, height: 100 };
const image: CanvasElement = { id: 'a', type: 'image', x: 100, y: 50, width: 80, height: 40, stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0 };

describe('selection resize handles', () => {
  test('corners keep the selection ratio and the opposite corner, with or without Shift', () => {
    for (let handle = 0; handle < 4; handle++) {
      for (const shift of [false, true]) {
        const pointer = { x: handle === 0 || handle === 3 ? -80 : 500, y: handle < 2 ? -160 : 290 };
        const to = resizeBounds(from, handle, pointer, shift);
        expect(to.width / to.height).toBeCloseTo(2);
        expect(resizeHandles(to)[(handle + 2) % 4]).toEqual(resizeHandles(from)[(handle + 2) % 4]);
      }
    }
  });
  test('a proportional group resize keeps every individual image ratio and relative placement', () => {
    const elements = [image, { ...image, id: 'b', x: 220, y: 100, width: 80, height: 50 }];
    const to = resizeBounds(from, 2, { x: 500, y: 180 });
    const changes = resizeElements(elements, new Set(['a', 'b']), from, to);
    for (const element of elements) {
      const resized = changes.get(element.id)!;
      expect(resized.width / resized.height).toBeCloseTo(element.width / element.height);
      expect(resized.x - from.x).toBeCloseTo((element.x - from.x) * 2);
      expect(resized.y - from.y).toBeCloseTo((element.y - from.y) * 2);
    }
  });
  test('edges change only their axis and keep the opposite edge fixed', () => {
    expect(resizeBounds(from, 4, { x: 1, y: 0 })).toEqual({ x: 100, y: 0, width: 200, height: 150 });
    expect(resizeBounds(from, 5, { x: 500, y: 999 })).toEqual({ ...from, width: 400 });
    expect(resizeBounds(from, 6, { x: 1, y: 250 })).toEqual({ ...from, height: 200 });
    expect(resizeBounds(from, 7, { x: 0, y: 999 })).toEqual({ x: 0, y: 50, width: 300, height: 100 });
  });
  test('Shift on every edge keeps proportions, centring expansion on the perpendicular axis', () => {
    const points = [{ x: 0, y: -50 }, { x: 500, y: 0 }, { x: 0, y: 250 }, { x: -100, y: 0 }];
    for (let handle = 4; handle < 8; handle++) {
      const to = resizeBounds(from, handle, points[handle - 4], true);
      expect(to.width).toBe(400);
      expect(to.height).toBe(200);
      if (handle % 2) expect(to.y + to.height / 2).toBe(100);
      else expect(to.x + to.width / 2).toBe(200);
    }
  });
  test('crossed handles and collapsed bounds stay finite and nonzero', () => {
    for (let handle = 0; handle < 8; handle++) {
      for (const bounds of [from, { x: 0, y: 0, width: 0, height: 0 }]) {
        const to = resizeBounds(bounds, handle, { x: -500, y: -200 }, true);
        expect(Object.values(to).every(Number.isFinite)).toBe(true);
        expect(to.width).toBeGreaterThanOrEqual(0);
        expect(to.height).toBeGreaterThanOrEqual(0);
      }
    }
  });
  test('a flat stroke scales along its nonzero axis without amplifying perpendicular motion', () => {
    expect(resizeBounds({ x: 0, y: 0, width: 100, height: 0 }, 2, { x: 200, y: 100 })).toEqual({ x: 0, y: 0, width: 200, height: 0 });
  });
});
