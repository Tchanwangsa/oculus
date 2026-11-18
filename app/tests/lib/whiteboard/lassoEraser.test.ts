import { describe, expect, test } from 'bun:test';
import { lassoSelection, pointInPolygon } from '@/lib/canvas/lasso';
import type { CanvasElement, Point } from '@/lib/canvas/model';

const look = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const ink = (id: string, x: number, y: number, points: Point[]): CanvasElement => ({ id, type: 'freedraw', x, y, width: 0, height: 0, ...look, points });
const box = (id: string, x: number, y: number, size = 20): CanvasElement => ({ id, type: 'rectangle', x, y, width: size, height: size, ...look });
const square: Point[] = [{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 100 }, { x: 0, y: 100 }];

describe('lasso', () => {
  test('a point is inside a loop or not, and a loop that crosses itself has a hole', () => {
    expect(pointInPolygon({ x: 50, y: 50 }, square)).toBe(true);
    expect(pointInPolygon({ x: 150, y: 50 }, square)).toBe(false);
    const eight: Point[] = [{ x: 0, y: 0 }, { x: 100, y: 100 }, { x: 100, y: 0 }, { x: 0, y: 100 }];
    expect(pointInPolygon({ x: 50, y: 20 }, eight)).toBe(false);
    expect(pointInPolygon({ x: 20, y: 50 }, eight)).toBe(true);
  });

  test('takes what is mostly inside the loop and leaves what it only clips', () => {
    const inside = ink('inside', 10, 10, [{ x: 0, y: 0 }, { x: 30, y: 30 }, { x: 60, y: 10 }]);
    const clipped = ink('clipped', 90, 50, [{ x: 0, y: 0 }, { x: 30, y: 0 }, { x: 60, y: 0 }, { x: 90, y: 0 }]);
    const mostly = ink('mostly', 40, 80, [{ x: 0, y: 0 }, { x: 20, y: 0 }, { x: 40, y: 0 }, { x: 70, y: 0 }]);
    const far = ink('far', 400, 400, [{ x: 0, y: 0 }, { x: 10, y: 10 }]);
    expect(lassoSelection([inside, clipped, mostly, far, box('shape', 40, 40), box('straddling', 90, 10)], square)).toEqual(['inside', 'mostly', 'shape']);
  });

  test('never takes a locked backdrop, and two points are not a loop', () => {
    expect(lassoSelection([{ ...box('page', 10, 10), locked: true }], square)).toEqual([]);
    expect(lassoSelection([box('shape', 40, 40)], square.slice(0, 2))).toEqual([]);
  });
});
