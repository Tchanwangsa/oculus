import { describe, expect, test } from 'bun:test';
import { bindArrowEnd, findBindingSnap, followBindings, moveElements, resolveArrow } from '../../../src/lib/canvas/bindings';
import { anchorPoint, nearestOutlinePoint, outlineAlong, outlineAnchor, outlineGauge } from '../../../src/lib/canvas/outline';
import { applyElementChanges } from '../../../src/lib/canvas/changes';
import { cloneCanvasElements } from '../../../src/lib/canvas/renderClipboard';
import type { CanvasElement, Point } from '../../../src/lib/canvas/model';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const shape = (id: string, type: CanvasElement['type'], x: number, y: number, width: number, height: number): CanvasElement => ({ id, type, x, y, width, height, ...base });
const arrowBetween = (id: string, a: Point, b: Point, extra: Partial<CanvasElement> = {}): CanvasElement =>
  ({ id, type: 'arrow', x: a.x, y: a.y, width: b.x - a.x, height: b.y - a.y, points: [{ x: 0, y: 0 }, { x: b.x - a.x, y: b.y - a.y }], ...base, ...extra });
const ends = (arrow: CanvasElement) => {
  const points = arrow.points!;
  return [{ x: arrow.x + points[0].x, y: arrow.y + points[0].y }, { x: arrow.x + points[points.length - 1].x, y: arrow.y + points[points.length - 1].y }];
};
const finite = (arrow: CanvasElement) => [arrow.x, arrow.y, arrow.width, arrow.height, ...arrow.points!.flatMap(p => [p.x, p.y])].every(Number.isFinite);
const near = (a: Point, b: Point, digits = 6) => { expect(a.x).toBeCloseTo(b.x, digits); expect(a.y).toBeCloseTo(b.y, digits); };
const centerOf = (el: CanvasElement) => ({ x: el.x + el.width / 2, y: el.y + el.height / 2 });
const offset = (el: CanvasElement, p: Point) => ({ x: p.x - centerOf(el).x, y: p.y - centerOf(el).y });

const rect = shape('r', 'rectangle', 0, 0, 100, 50);
const ellipse = shape('e', 'ellipse', 0, 0, 100, 50);
const diamond = shape('d', 'diamond', 0, 0, 100, 50);
const text = { ...shape('t', 'text', 0, 0, 100, 50), text: 'Hello', fontSize: 20 };

describe('outline geometry', () => {
  test('the ray from the centre meets each kind of outline', () => {
    for (const el of [rect, ellipse, diamond, text]) {
      near(outlineAlong(el, { x: 400, y: 25 }), { x: 100, y: 25 });
      near(outlineAlong(el, { x: 50, y: -300 }), { x: 50, y: 0 });
    }
    near(outlineAlong(rect, { x: 150, y: 75 }), { x: 100, y: 50 });
    near(outlineAlong(text, { x: 150, y: 75 }), { x: 100, y: 50 });
    near(outlineAlong(diamond, { x: 150, y: 75 }), { x: 75, y: 37.5 });
    const onEllipse = outlineAlong(ellipse, { x: 150, y: 75 });
    expect(outlineGauge(ellipse, offset(ellipse, onEllipse))).toBeCloseTo(1, 9);
    expect(onEllipse.x - 50).toBeCloseTo(2 * (onEllipse.y - 25), 9);
  });

  test('nearest outline point from outside and inside', () => {
    near(nearestOutlinePoint(rect, { x: 140, y: 30 }), { x: 100, y: 30 });
    near(nearestOutlinePoint(rect, { x: 150, y: 90 }), { x: 100, y: 50 });
    near(nearestOutlinePoint(rect, { x: 50, y: 10 }), { x: 50, y: 0 });
    near(nearestOutlinePoint(text, { x: 95, y: 25 }), { x: 100, y: 25 });
    near(nearestOutlinePoint(diamond, { x: 100, y: 0 }), { x: 90, y: 20 });
    for (const p of [{ x: 120, y: 60 }, { x: 70, y: 20 }, { x: -30, y: 5 }, { x: 50, y: 80 }]) {
      const q = nearestOutlinePoint(ellipse, p);
      expect(outlineGauge(ellipse, offset(ellipse, q))).toBeCloseTo(1, 6);
      let best = Infinity;
      for (let i = 0; i < 20000; i++) {
        const t = (i / 20000) * Math.PI * 2;
        best = Math.min(best, Math.hypot(50 + 50 * Math.cos(t) - p.x, 25 + 25 * Math.sin(t) - p.y));
      }
      expect(Math.hypot(q.x - p.x, q.y - p.y)).toBeLessThan(best + 1e-3);
    }
  });

  test('an anchor keeps its outline point through a resize', () => {
    const anchor = outlineAnchor(ellipse, outlineAlong(ellipse, { x: 150, y: 75 }));
    const resized = { ...ellipse, x: 20, width: 300, height: 80 };
    expect(outlineGauge(resized, offset(resized, anchorPoint(resized, anchor)))).toBeCloseTo(1, 9);
    near(anchorPoint({ ...rect, width: 200 }, { x: 0.5, y: 0 }), { x: 100, y: 0 });
  });
});

describe('snapping an arrow end', () => {
  test('the central zone binds to the centre', () => {
    const snap = findBindingSnap([rect], { x: 55, y: 28 }, 1);
    expect(snap?.binding).toEqual({ elementId: 'r', mode: 'center' });
    near(snap!.point, { x: 50, y: 25 });
  });

  test('near an edge midpoint snaps to it; elsewhere to the nearest outline point', () => {
    const mid = findBindingSnap([rect], { x: 58, y: -6 }, 1)!;
    expect(mid.binding).toEqual({ elementId: 'r', mode: 'edge', anchor: { x: 0.5, y: 0 } });
    const edge = findBindingSnap([rect], { x: 85, y: -6 }, 1)!;
    near(edge.point, { x: 85, y: 0 });
    expect(edge.binding).toMatchObject({ mode: 'edge', anchor: { x: 0.85, y: 0 } });
  });

  test('ranges are in screen pixels, the topmost bindable shape wins, and lines never bind', () => {
    expect(findBindingSnap([rect], { x: 120, y: 25 }, 1)).toBeNull();
    expect(findBindingSnap([rect], { x: 120, y: 25 }, 0.5)?.target.id).toBe('r');
    const top = shape('top', 'ellipse', 40, 0, 100, 50), line = shape('l', 'line', 0, 0, 100, 50);
    expect(findBindingSnap([rect, top, line], { x: 90, y: 25 }, 1)?.target.id).toBe('top');
  });

  test('both ends never bind to one shape by its centre', () => {
    const drawn = arrowBetween('a', { x: 300, y: 25 }, { x: 50, y: 25 }, { startBinding: { elementId: 'r', mode: 'center' } });
    const { arrow } = bindArrowEnd(drawn, 'end', { x: 50, y: 25 }, [rect], 1);
    expect(arrow.endBinding?.mode).toBe('edge');
  });

  test('holding the modifier or leaving a shape unbinds the end', () => {
    const drawn = arrowBetween('a', { x: 300, y: 25 }, { x: 50, y: 25 }, { endBinding: { elementId: 'r', mode: 'center' } });
    expect(bindArrowEnd(drawn, 'end', { x: 50, y: 25 }, [rect], 1, { free: true }).arrow.endBinding).toBeUndefined();
    expect(bindArrowEnd(drawn, 'end', { x: 500, y: 25 }, [rect], 1).arrow.endBinding).toBeUndefined();
  });

  test('a shape being previewed is found where it is now', () => {
    const moved = { ...rect, x: 400 };
    expect(findBindingSnap([rect], { x: 450, y: 25 }, 1, { live: new Map([['r', moved]]) })?.target).toBe(moved);
    expect(findBindingSnap([rect], { x: 50, y: 25 }, 1, { live: new Map([['r', null]]) })).toBeNull();
  });
});

describe('placing bound ends', () => {
  test('centre binding stops at the outline, pulled back by 4 + strokeWidth/2', () => {
    const arrow = arrowBetween('a', { x: 300, y: 25 }, { x: 50, y: 25 }, { endBinding: { elementId: 'r', mode: 'center' } });
    const [start, end] = ends(resolveArrow(arrow, id => id === 'r' ? rect : undefined));
    near(start, { x: 300, y: 25 }); near(end, { x: 105, y: 25 });
    const thick = resolveArrow({ ...arrow, strokeWidth: 6 }, () => rect);
    near(ends(thick)[1], { x: 107, y: 25 });
  });

  test('edge binding sits at its anchor, pulled toward the other end', () => {
    const arrow = arrowBetween('a', { x: 50, y: -200 }, { x: 0, y: 0 }, { endBinding: { elementId: 'r', mode: 'edge', anchor: { x: 0.5, y: 0 } } });
    near(ends(resolveArrow(arrow, () => rect))[1], { x: 50, y: -5 });
  });

  test('two bound ends aim at each other', () => {
    const a = shape('a', 'rectangle', 0, 0, 100, 100), b = shape('b', 'ellipse', 300, 0, 100, 100);
    const arrow = arrowBetween('x', { x: 0, y: 0 }, { x: 1, y: 1 }, { startBinding: { elementId: 'a', mode: 'center' }, endBinding: { elementId: 'b', mode: 'center' } });
    const [start, end] = ends(resolveArrow(arrow, id => id === 'a' ? a : b));
    near(start, { x: 105, y: 50 }); near(end, { x: 295, y: 50 });
  });

  test('an unchanged arrow comes back as itself', () => {
    const arrow = resolveArrow(arrowBetween('a', { x: 300, y: 25 }, { x: 0, y: 0 }, { endBinding: { elementId: 'r', mode: 'center' } }), () => rect);
    expect(resolveArrow(arrow, () => rect)).toBe(arrow);
  });

  test('degenerate targets and ends stay finite', () => {
    const point = shape('p', 'rectangle', 40, 40, 0, 0), flat = shape('f', 'ellipse', 0, 0, 100, 0), dot = shape('d', 'diamond', 10, 10, 0, 0);
    for (const target of [point, flat, dot, rect]) {
      for (const other of [{ x: 300, y: 25 }, { x: 50, y: 25 }, { x: 40, y: 40 }, { x: 60, y: 30 }]) {
        for (const binding of [{ elementId: target.id, mode: 'center' as const }, { elementId: target.id, mode: 'edge' as const, anchor: { x: 0.5, y: 0 } }]) {
          expect(finite(resolveArrow(arrowBetween('a', other, other, { endBinding: binding }), () => target))).toBe(true);
          expect(finite(resolveArrow(arrowBetween('a', other, other, { startBinding: binding, endBinding: binding }), () => target))).toBe(true);
        }
      }
    }
    expect(nearestOutlinePoint(flat, { x: 30, y: 10 })).toEqual({ x: 30, y: 0 });
    expect(Number.isFinite(nearestOutlinePoint(point, { x: 0, y: 0 }).x)).toBe(true);
  });

  test('the other end inside a centre-bound target leaves the end at the centre', () => {
    const arrow = arrowBetween('a', { x: 70, y: 25 }, { x: 0, y: 0 }, { endBinding: { elementId: 'r', mode: 'center' } });
    near(ends(resolveArrow(arrow, () => rect))[1], { x: 50, y: 25 });
  });

  test('centre and edge on the same shape run from the centre to the edge', () => {
    const arrow = arrowBetween('a', { x: 0, y: 0 }, { x: 0, y: 0 }, { startBinding: { elementId: 'r', mode: 'center' }, endBinding: { elementId: 'r', mode: 'edge', anchor: { x: 1, y: 0.5 } } });
    const [start, end] = ends(resolveArrow(arrow, () => rect));
    near(start, { x: 50, y: 25 }); near(end, { x: 95, y: 25 });
  });
});

describe('following shapes', () => {
  const target = shape('r', 'rectangle', 0, 0, 100, 50);
  const bound = resolveArrow(arrowBetween('a', { x: 300, y: 25 }, { x: 50, y: 25 }, { endBinding: { elementId: 'r', mode: 'center' } }), () => target);
  const scene = [target, bound];

  test('a moved shape carries its bound arrow; the free end stays', () => {
    const changes = followBindings(scene, moveElements(scene, new Set(['r']), 0, 100));
    const [start, end] = ends(changes.get('a')!);
    near(start, { x: 300, y: 25 });
    expect(Math.hypot(end.x - 300, end.y - 25)).toBeLessThan(Math.hypot(50 - 300, 125 - 25));
    expect(changes.get('a')!.endBinding).toEqual({ elementId: 'r', mode: 'center' });
  });

  test('a resized shape carries its bound arrow', () => {
    const changes = followBindings(scene, new Map([['r', { ...target, width: 200 }]]));
    near(ends(changes.get('a')!)[1], { x: 205, y: 25 });
  });

  test('an untouched scene returns the same changes', () => {
    const changes = new Map([['other', shape('other', 'rectangle', 500, 500, 10, 10)]]);
    expect(followBindings(scene, changes)).toBe(changes);
  });

  test('moving an arrow without its target unbinds it; moving both keeps the binding', () => {
    const alone = followBindings(scene, moveElements(scene, new Set(['a']), 10, 10)).get('a')!;
    expect(alone.endBinding).toBeUndefined();
    near(ends(alone)[1], { x: 115, y: 35 });
    const both = followBindings(scene, moveElements(scene, new Set(['a', 'r']), 10, 10)).get('a')!;
    expect(both.endBinding).toEqual({ elementId: 'r', mode: 'center' });
    near(ends(both)[0], { x: 310, y: 35 }); near(ends(both)[1], { x: 115, y: 35 });
  });

  test('deleting a target strips bindings to it and keeps the geometry', () => {
    const next = applyElementChanges(scene, followBindings(scene, new Map([['r', null]])));
    expect(next.map(el => el.id)).toEqual(['a']);
    expect(next[0].endBinding).toBeUndefined();
    expect(ends(next[0])).toEqual(ends(bound));
  });

  test('a newly added arrow (drawn or pasted) is placed against the scene', () => {
    const drawn = arrowBetween('n', { x: 300, y: 25 }, { x: 50, y: 25 }, { endBinding: { elementId: 'r', mode: 'center' } });
    near(ends(followBindings(scene, new Map([['n', drawn]])).get('n')!)[1], { x: 105, y: 25 });
  });
});

describe('cloning bound arrows', () => {
  const a = shape('a', 'rectangle', 0, 0, 100, 50), b = shape('b', 'rectangle', 300, 0, 100, 50);
  const arrow = arrowBetween('x', { x: 105, y: 25 }, { x: 295, y: 25 }, { startBinding: { elementId: 'a', mode: 'center' }, endBinding: { elementId: 'b', mode: 'edge', anchor: { x: 0, y: 0.5 } } });

  test('bindings to shapes cloned together point at the clones', () => {
    let n = 0;
    const [ca, cb, cx] = cloneCanvasElements([a, b, arrow], { x: 10, y: 10 }, () => `c${++n}`);
    expect([ca.id, cb.id, cx.id]).toEqual(['c1', 'c2', 'c3']);
    expect(cx.startBinding).toEqual({ elementId: 'c1', mode: 'center' });
    expect(cx.endBinding).toEqual({ elementId: 'c2', mode: 'edge', anchor: { x: 0, y: 0.5 } });
    expect(arrow.startBinding?.elementId).toBe('a');
  });

  test('bindings to shapes left behind are dropped', () => {
    let n = 0;
    const [ca, cx] = cloneCanvasElements([a, arrow], undefined, () => `c${++n}`);
    expect(cx.startBinding).toEqual({ elementId: ca.id, mode: 'center' });
    expect('endBinding' in cx).toBe(false);
  });
});

describe('elbow ends inside a shape', () => {
  const big = shape('big', 'rectangle', 0, 0, 200, 120);
  const drawn = (extra: Partial<CanvasElement>) => arrowBetween('x', { x: 400, y: 300 }, { x: 140, y: 30 }, extra);
  test('bind by the side facing the other end, not the nearest edge', () => {
    expect(bindArrowEnd(drawn({ arrowType: 'elbow' }), 'end', { x: 140, y: 30 }, [big], 1).arrow.endBinding).toEqual({ elementId: 'big', mode: 'center' });
    expect(bindArrowEnd(drawn({}), 'end', { x: 140, y: 30 }, [big], 1).arrow.endBinding?.mode).toBe('edge');
  });
  test('still bind to the edge near the outline', () => {
    expect(bindArrowEnd(drawn({ arrowType: 'elbow' }), 'end', { x: 140, y: 8 }, [big], 1).arrow.endBinding?.mode).toBe('edge');
  });
});
