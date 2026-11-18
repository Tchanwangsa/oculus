import { describe, expect, test } from 'bun:test';
import { arrowheadShape, elementArrowheads, elementBounds, elementHitTest, selectionBounds, screenToWorld, strokeDash, worldToScreen, zoomAt, simplifyPolyline } from '../../../src/lib/canvas/geometry';
import type { CanvasElement } from '../../../src/lib/canvas/model';

const rect: CanvasElement = { id: 'a', type: 'rectangle', x: 100, y: 50, width: -80, height: 40, stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 1 };

describe('canvas geometry', () => {
  test('normalizes dimensions and local stroke coordinates in selection bounds', () => {
    expect(elementBounds(rect)).toEqual({ x: 20, y: 50, width: 80, height: 40 });
    const stroke = { ...rect, type: 'freedraw' as const, points: [{ x: -20, y: -10 }, { x: 120, y: 100 }] };
    expect(elementBounds(stroke)).toEqual({ x: 80, y: 40, width: 140, height: 110 });
    expect(selectionBounds([rect, stroke])).toEqual({ x: 20, y: 40, width: 200, height: 110 });
    expect(selectionBounds([])).toBeNull();
  });
  test('zoom preserves the world point under the pointer and extension fields', () => {
    const v = { x: 130, y: -60, zoom: 0.4, grid: 'dots' }, p = { x: 311, y: 211 };
    const next = zoomAt(v, p, 3.2);
    expect(screenToWorld(p, next)).toEqual(screenToWorld(p, v));
    expect(worldToScreen(screenToWorld(p, v), v)).toEqual(p);
    expect(next.grid).toBe('dots');
    expect(zoomAt(v, p, 100).zoom).toBe(8);
  });
  test('unfilled shapes only select their outlines; text selects its box', () => {
    expect(elementHitTest(rect, { x: 60, y: 70 }, 1)).toBeFalse();
    expect(elementHitTest(rect, { x: 21, y: 70 }, 1)).toBeTrue();
    expect(elementHitTest({ ...rect, type: 'text' }, { x: 60, y: 70 }, 1)).toBeTrue();
  });
  test('a polyline does not select empty space in its bounding box', () => {
    const el = { ...rect, type: 'line' as const, x: 0, y: 0, points: [{ x: 0, y: 0 }, { x: 100, y: 100 }] };
    expect(elementHitTest(el, { x: 50, y: 50 }, 2)).toBeTrue();
    expect(elementHitTest(el, { x: 10, y: 90 }, 2)).toBeFalse();
  });
  test('simplification retains corners, endpoints, and closed paths', () => {
    const points = [{ x: 0, y: 0 }, { x: 1, y: 0.1 }, { x: 2, y: 0 }, { x: 2, y: 2 }];
    expect(simplifyPolyline(points, 0.2)).toEqual([points[0], points[2], points[3]]);
    expect(simplifyPolyline([points[0], points[3], points[0]], 0.2)).toHaveLength(3);
  });
  test('a label makes a hollow shape selectable inside', () => {
    for (const type of ['rectangle', 'ellipse', 'diamond'] as const) {
      expect(elementHitTest({ ...rect, type }, { x: 60, y: 70 }, 1)).toBeFalse();
      expect(elementHitTest({ ...rect, type, text: 'Label' }, { x: 60, y: 70 }, 1)).toBeTrue();
      expect(elementHitTest({ ...rect, type, text: '  ' }, { x: 60, y: 70 }, 1)).toBeFalse();
    }
  });
  test('arrowheads hit at the ends that have one', () => {
    const arrow: CanvasElement = { ...rect, type: 'arrow', x: 0, y: 0, points: [{ x: 0, y: 0 }, { x: 100, y: 0 }] };
    const nearEnd = { x: 88, y: 6 }, nearStart = { x: 12, y: 6 };
    expect(elementHitTest(arrow, nearEnd, 1)).toBeTrue();
    expect(elementHitTest(arrow, nearStart, 1)).toBeFalse();
    expect(elementHitTest({ ...arrow, endArrowhead: 'none', startArrowhead: 'bar' }, nearEnd, 1)).toBeFalse();
    expect(elementHitTest({ ...arrow, endArrowhead: 'none', startArrowhead: 'bar' }, nearStart, 1)).toBeTrue();
  });
});

describe('arrowhead descriptors', () => {
  const line = [{ x: 0, y: 0 }, { x: 100, y: 0 }];
  const round = (head: ReturnType<typeof arrowheadShape>) => JSON.parse(JSON.stringify(head, (_, v) => typeof v === 'number' ? Math.round(v * 1000) / 1000 : v));
  test('every type shares the tip, size and wing pair', () => {
    for (const type of ['arrow', 'triangle', 'dot', 'bar', 'diamond'] as const) {
      expect(round(arrowheadShape(line, 'end', type))).toMatchObject({ type, tip: { x: 100, y: 0 }, size: 16, wings: [{ x: 86.144, y: 8 }, { x: 86.144, y: -8 }] });
    }
    expect(arrowheadShape(line, 'end', 'none')).toBeNull();
    expect(arrowheadShape([line[0]], 'end', 'arrow')).toBeNull();
    expect(arrowheadShape([{ x: 0, y: 0 }, { x: 30, y: 0 }], 'end', 'arrow')!.size).toBe(10);
  });
  test('each type has its own outline', () => {
    expect(round(arrowheadShape(line, 'end', 'arrow'))).toMatchObject({ closed: false, points: [{ x: 86.144, y: 8 }, { x: 100, y: 0 }, { x: 86.144, y: -8 }] });
    expect(round(arrowheadShape(line, 'end', 'triangle'))).toMatchObject({ closed: true, points: [{ x: 86.144, y: 8 }, { x: 100, y: 0 }, { x: 86.144, y: -8 }] });
    expect(round(arrowheadShape(line, 'end', 'bar'))).toMatchObject({ closed: false, points: [{ x: 100, y: -8 }, { x: 100, y: 8 }] });
    expect(round(arrowheadShape(line, 'end', 'diamond'))).toMatchObject({ closed: true, points: [{ x: 100, y: 0 }, { x: 92, y: 5.333 }, { x: 84, y: 0 }, { x: 92, y: -5.333 }] });
    expect(round(arrowheadShape(line, 'end', 'dot'))).toMatchObject({ closed: false, points: [], dot: { center: { x: 93.6, y: 0 }, radius: 6.4 } });
  });
  test('start heads point backwards along the first segment; defaults are a plain end arrow', () => {
    expect(round(arrowheadShape(line, 'start', 'arrow'))).toMatchObject({ tip: { x: 0, y: 0 }, wings: [{ x: 13.856, y: -8 }, { x: 13.856, y: 8 }] });
    const arrow: CanvasElement = { ...rect, type: 'arrow', points: line };
    expect(elementArrowheads(arrow).map(head => [head.type, head.tip.x])).toEqual([['arrow', 100]]);
    expect(elementArrowheads({ ...arrow, startArrowhead: 'dot', endArrowhead: 'none' }).map(head => [head.type, head.tip.x])).toEqual([['dot', 0]]);
    expect(elementArrowheads({ ...arrow, type: 'line', startArrowhead: 'dot' })).toEqual([]);
  });
  test('dash lengths scale with stroke width; freehand stays solid', () => {
    expect(strokeDash(rect)).toEqual([]);
    expect(strokeDash({ ...rect, strokeStyle: 'dashed', strokeWidth: 1 })).toEqual([6, 4]);
    expect(strokeDash({ ...rect, strokeStyle: 'dashed', strokeWidth: 3 })).toEqual([12, 9]);
    expect(strokeDash({ ...rect, strokeStyle: 'dotted' })).toEqual([0, 6]);
    expect(strokeDash({ ...rect, type: 'freedraw', strokeStyle: 'dashed' })).toEqual([]);
  });
});
