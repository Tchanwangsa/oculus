import { describe, expect, test } from 'bun:test';
import { pointDrag, pointHandleAt, pointHandles } from '../../../src/lib/canvas/arrow/handles';
import { endAt, insertPoint, movePoint, removePoint, worldPoints } from '../../../src/lib/canvas/arrow/points';
import { bindArrowEnd, followBindings, resolveArrow } from '../../../src/lib/canvas/bindings';
import { elementBounds } from '../../../src/lib/canvas/geometry';
import { stylePatchChanges } from '../../../src/lib/canvas/labels';
import { resizeElements } from '../../../src/lib/canvas/resize';
import { adoptElementStyle, DEFAULT_DRAWING_STYLE, newElementStyle } from '../../../src/lib/canvas/style';
import type { CanvasElement, Point } from '../../../src/lib/canvas/model';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const through = (world: Point[], extra: Partial<CanvasElement> = {}): CanvasElement => {
  const [s, e] = [world[0], world[world.length - 1]];
  return { id: 'a', type: 'arrow', x: s.x, y: s.y, width: e.x - s.x, height: e.y - s.y, points: world.map(p => ({ x: p.x - s.x, y: p.y - s.y })), ...base, ...extra };
};
const zigzag = [{ x: 10, y: 10 }, { x: 110, y: 10 }, { x: 110, y: 110 }, { x: 210, y: 110 }];
const near = (a: Point, b: Point) => { expect(a.x).toBeCloseTo(b.x, 9); expect(a.y).toBeCloseTo(b.y, 9); };

describe('point edits', () => {
  test('moving a point keeps the others and re-bases on the first point', () => {
    const moved = movePoint(through(zigzag), 1, { x: 130, y: -20 });
    expect(worldPoints(moved)).toEqual([zigzag[0], { x: 130, y: -20 }, zigzag[2], zigzag[3]]);
    expect(moved.points![0]).toEqual({ x: 0, y: 0 });
    const start = movePoint(through(zigzag), 0, { x: -40, y: 0 });
    expect(worldPoints(start)).toEqual([{ x: -40, y: 0 }, ...zigzag.slice(1)]);
    expect(start).toMatchObject({ x: -40, y: 0, width: 250, height: 110 });
  });
  test('an end drag keeps the interior points', () => {
    const moved = movePoint(through(zigzag), 3, { x: 300, y: 200 });
    expect(worldPoints(moved)).toEqual([...zigzag.slice(0, 3), { x: 300, y: 200 }]);
  });
  test('Shift snaps the dragged segment to 45° from its previous point (the next one, for the start)', () => {
    near(worldPoints(movePoint(through(zigzag), 2, { x: 200, y: 20 }, true))[2], { x: 110 + Math.hypot(90, 10), y: 10 });
    const snapped = worldPoints(movePoint(through(zigzag), 2, { x: 160, y: 62 }, true))[2];
    expect(snapped.x - 110).toBeCloseTo(snapped.y - 10, 9);
    near(worldPoints(movePoint(through(zigzag), 0, { x: 112, y: -60 }, true))[0], { x: 110, y: 10 - Math.hypot(2, 70) });
  });
  test('insert and remove interior points; the ends cannot be removed', () => {
    const inserted = insertPoint(through(zigzag), 0, { x: 60, y: 0 });
    expect(worldPoints(inserted)).toEqual([zigzag[0], { x: 60, y: 0 }, ...zigzag.slice(1)]);
    expect(worldPoints(removePoint(inserted, 1))).toEqual(zigzag);
    const arrow = through(zigzag);
    expect(removePoint(arrow, 0)).toBe(arrow);
    expect(removePoint(arrow, 3)).toBe(arrow);
    expect([0, 1, 3].map(i => endAt(arrow, i))).toEqual(['start', undefined, 'end']);
  });
});

describe('point handles', () => {
  test('a sharp arrow has every point and a midpoint per long-enough segment', () => {
    const arrow = through([{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 110, y: 0 }]);
    const handles = pointHandles(arrow, 1);
    expect(handles.filter(h => h.kind === 'point').map(h => h.at)).toEqual([{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 110, y: 0 }]);
    expect(handles.filter(h => h.kind === 'midpoint')).toEqual([{ kind: 'midpoint', segment: 0, at: { x: 50, y: 0 } }]);
    expect(pointHandles(arrow, 4).filter(h => h.kind === 'midpoint')).toHaveLength(2);
  });
  test('a curved arrow puts its midpoints on the curve', () => {
    const arrow = through(zigzag, { arrowType: 'curved' });
    const mid = pointHandles(arrow, 1).find(h => h.kind === 'midpoint' && h.segment === 0)!;
    expect(mid.at.x).toBeCloseTo(60, 9);
    expect(mid.at.y).toBeLessThan(10 - 1);
  });
  test('lines show only their ends; elbow arrows their ends and a segment handle per long-enough segment', () => {
    const elbow = pointHandles(through(zigzag, { arrowType: 'elbow' }), 1);
    expect(elbow.filter(h => h.kind === 'point').map(h => h.kind === 'point' && h.index)).toEqual([0, 3]);
    expect(elbow.filter(h => h.kind === 'segment')).toEqual([
      { kind: 'segment', segment: 0, at: { x: 60, y: 10 } }, { kind: 'segment', segment: 1, at: { x: 110, y: 60 } }, { kind: 'segment', segment: 2, at: { x: 160, y: 110 } },
    ]);
    expect(elbow.some(h => h.kind === 'midpoint')).toBe(false);
    expect(pointHandles(through(zigzag, { arrowType: 'elbow' }), 0.25).filter(h => h.kind === 'segment')).toHaveLength(0);
    expect(pointHandles(through(zigzag, { type: 'line' }), 1).map(h => h.kind === 'point' && h.index)).toEqual([0, 3]);
  });
  test('a point wins over a midpoint near it, within range on screen', () => {
    const arrow = through(zigzag);
    expect(pointHandleAt(arrow, { x: 112, y: 12 }, 1)).toMatchObject({ kind: 'point', index: 1 });
    expect(pointHandleAt(arrow, { x: 62, y: 12 }, 1)).toMatchObject({ kind: 'midpoint', segment: 0 });
    expect(pointHandleAt(arrow, { x: 75, y: 10 }, 1)).toBeNull();
    expect(pointHandleAt(arrow, { x: 75, y: 10 }, 0.5)).toMatchObject({ kind: 'midpoint', segment: 0 });
  });
  test('dragging a midpoint inserts its point and moves that one', () => {
    const arrow = through(zigzag);
    const { base, index } = pointDrag(arrow, pointHandleAt(arrow, { x: 60, y: 10 }, 1)!);
    expect(index).toBe(1);
    expect(worldPoints(base)).toEqual([zigzag[0], { x: 60, y: 10 }, ...zigzag.slice(1)]);
    expect(worldPoints(movePoint(base, index, { x: 60, y: -30 }))[1]).toEqual({ x: 60, y: -30 });
    expect(pointDrag(arrow, { kind: 'point', index: 2, at: zigzag[2] })).toEqual({ base: arrow, index: 2 });
  });
});

describe('arrow type switching', () => {
  const arrow = through(zigzag);
  const retype = (el: CanvasElement, arrowType: 'sharp' | 'curved' | 'elbow') => stylePatchChanges([el], new Set([el.id]), { arrowType }).get(el.id);
  test('sharp and curved keep their points', () => {
    const curved = retype(arrow, 'curved')!;
    expect(curved.arrowType).toBe('curved');
    expect(worldPoints(curved)).toEqual(zigzag);
    expect(worldPoints(retype(curved, 'sharp')!)).toEqual(zigzag);
  });
  test('leaving elbow keeps only the two ends', () => {
    const elbow = { ...arrow, arrowType: 'elbow' as const };
    for (const type of ['sharp', 'curved'] as const) expect(worldPoints(retype(elbow, type)!)).toEqual([zigzag[0], zigzag[3]]);
  });
  test('new arrows take the type from the drawing style, and selecting one adopts its type', () => {
    expect(newElementStyle('arrow', DEFAULT_DRAWING_STYLE)).not.toHaveProperty('arrowType');
    expect(newElementStyle('arrow', { ...DEFAULT_DRAWING_STYLE, arrowType: 'curved' })).toMatchObject({ arrowType: 'curved' });
    expect(newElementStyle('line', { ...DEFAULT_DRAWING_STYLE, arrowType: 'curved' })).not.toHaveProperty('arrowType');
    expect(adoptElementStyle(DEFAULT_DRAWING_STYLE, { ...arrow, arrowType: 'elbow' }).arrowType).toBe('elbow');
  });
  test('a patch that changes nothing commits nothing; other kinds ignore the field', () => {
    expect(retype(arrow, 'sharp')).toBeUndefined();
    expect(retype({ ...arrow, type: 'line' }, 'curved')).toBeUndefined();
  });
});

describe('bound arrows with interior points', () => {
  const box = (id: string, x: number, y: number): CanvasElement => ({ id, type: 'rectangle', x, y, width: 100, height: 60, ...base });
  const left = box('l', -200, -30), right = box('r', 300, 200);
  const bound = resolveArrow(through([{ x: -150, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 230 }, { x: 350, y: 230 }], {
    arrowType: 'curved', startBinding: { elementId: 'l', mode: 'center' }, endBinding: { elementId: 'r', mode: 'center' },
  }), id => ({ l: left, r: right })[id]);
  test('each bound end aims at its neighbouring point', () => {
    const points = worldPoints(bound);
    near(points[0], { x: -100 + 5, y: 0 });
    near(points[3], { x: 300 - 5, y: 230 });
  });
  test('interior points stay put when a target moves', () => {
    const elements = [left, right, bound];
    const moved = followBindings(elements, new Map([['r', { ...right, y: 300 }]])).get('a')!;
    const points = worldPoints(moved);
    expect(points.slice(1, 3)).toEqual(worldPoints(bound).slice(1, 3));
    // The centre's ray toward the neighbour leaves the box at (300, 310), then the end steps 5 back along it.
    const back = 5 / Math.hypot(250, 100);
    near(points[3], { x: 300 - 250 * back, y: 310 - 100 * back });
    expect(moved.arrowType).toBe('curved');
  });
  test('dragging a bound end rebinds it and keeps the bends', () => {
    const elements = [left, right, bound];
    const dragged = movePoint(bound, 3, { x: 600, y: 600 });
    const { arrow } = bindArrowEnd(dragged, 'end', { x: 600, y: 600 }, elements, 1);
    expect(arrow.endBinding).toBeUndefined();
    const placed = followBindings(elements, new Map([['a', arrow]])).get('a')!;
    expect(worldPoints(placed).slice(1)).toEqual([...worldPoints(bound).slice(1, 3), { x: 600, y: 600 }]);
  });
});

describe('resizing a selection with multi-point arrows', () => {
  test('every point scales and a curve keeps fitting the scaled bounds', () => {
    const arrow = through(zigzag, { arrowType: 'curved' });
    const from = elementBounds(arrow), to = { x: from.x + 50, y: from.y - 20, width: from.width * 2, height: from.height / 2 };
    const resized = resizeElements([arrow], new Set(['a']), from, to).get('a')!;
    expect(resized.points).toHaveLength(4);
    const after = elementBounds(resized);
    // Flattening is within a quarter unit of the curve, and the scaled curve is the curve scaled.
    for (const key of ['x', 'y', 'width', 'height'] as const) expect(Math.abs(after[key] - to[key])).toBeLessThan(0.25);
    for (const [i, p] of worldPoints(resized).entries()) near(p, { x: to.x + (zigzag[i].x - from.x) * 2, y: to.y + (zigzag[i].y - from.y) / 2 });
  });
});
