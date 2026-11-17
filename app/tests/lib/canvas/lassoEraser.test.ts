import { describe, expect, test } from 'bun:test';
import { erasePartially } from '@/lib/canvas/eraser';
import { lassoSelection, pointInPolygon } from '@/lib/canvas/lasso';
import type { CanvasElement, Point } from '@/lib/canvas/model';

const look = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const ink = (id: string, x: number, y: number, points: Point[]): CanvasElement => ({ id, type: 'freedraw', x, y, width: 0, height: 0, ...look, points });
const box = (id: string, x: number, y: number, size = 20): CanvasElement => ({ id, type: 'rectangle', x, y, width: size, height: size, ...look });
const square: Point[] = [{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 100 }, { x: 0, y: 100 }];
const ids = (() => { let n = 0; return () => `new${++n}`; });

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

describe('partial eraser', () => {
  const line = ink('line', 0, 100, [{ x: 0, y: 0 }, { x: 200, y: 0 }]);

  test('rubbing across the middle of a stroke leaves two pieces, the first keeping its ID', () => {
    const changes = erasePartially([line], { x: 100, y: 80 }, { x: 100, y: 120 }, 10, ids());
    expect([...changes.keys()]).toEqual(['line', 'new1']);
    const first = changes.get('line')!, second = changes.get('new1')!;
    expect(first.points![0]).toEqual({ x: 0, y: 0 });
    // The eraser reaches its radius plus half the stroke's width either side of x = 100.
    expect(first.points!.at(-1)!.x).toBeLessThan(89);
    expect(first.points!.at(-1)!.x).toBeGreaterThan(80);
    expect(second.points![0].x).toBeGreaterThan(111);
    expect(second.points!.at(-1)).toEqual({ x: 200, y: 0 });
    expect(second).toMatchObject({ type: 'freedraw', x: 0, y: 100, stroke: '#111111', strokeWidth: 2, width: 200, height: 0 });
  });

  test('rubbing an end shortens the stroke, and rubbing all of it removes it', () => {
    const end = erasePartially([line], { x: 200, y: 100 }, { x: 200, y: 100 }, 10, ids());
    expect([...end.keys()]).toEqual(['line']);
    expect(end.get('line')!.points!.at(-1)!.x).toBeLessThan(189);
    expect(erasePartially([line], { x: -10, y: 100 }, { x: 210, y: 100 }, 5, ids()).get('line')).toBeNull();
  });

  test('a fast drag rubs out everything between two samples', () => {
    const rows = [ink('a', 0, 0, [{ x: 0, y: 0 }, { x: 0, y: 100 }]), ink('b', 50, 0, [{ x: 0, y: 0 }, { x: 0, y: 100 }]), ink('c', 100, 0, [{ x: 0, y: 0 }, { x: 0, y: 100 }])];
    const changes = erasePartially(rows, { x: -20, y: 50 }, { x: 120, y: 50 }, 6, ids());
    expect([...changes.values()].filter(Boolean)).toHaveLength(6);
  });

  test('strokes out of reach, shapes and locked ink are left alone', () => {
    const shape = box('shape', 90, 90), locked = { ...line, id: 'locked', locked: true };
    expect(erasePartially([line, shape, locked], { x: 100, y: 300 }, { x: 100, y: 320 }, 10, ids()).size).toBe(0);
    expect([...erasePartially([shape, locked], { x: 100, y: 80 }, { x: 100, y: 120 }, 10, ids()).keys()]).toEqual([]);
  });
});
