import { describe, expect, test } from 'bun:test';
import { routeElbow } from '../../../src/lib/canvas/elbow';
import { elbowEnds, type RouteEnd } from '../../../src/lib/canvas/elbow/ends';
import { elbowRoute, fallbackRoute, routeObstacles } from '../../../src/lib/canvas/elbow/route';
import { boxOf, segmentEntersBox } from '../../../src/lib/canvas/elbow/grid';
import { DOWN, LEFT, RIGHT, UP, flip, vectorHeading, type Heading } from '../../../src/lib/canvas/elbow/headings';
import { bindArrowEnd, followBindings, moveElements } from '../../../src/lib/canvas/bindings';
import { movePoint, worldPoints } from '../../../src/lib/canvas/arrow/points';
import { stylePatchChanges } from '../../../src/lib/canvas/labels';
import { cloneCanvasElements } from '../../../src/lib/canvas/renderClipboard';
import { resizeElements } from '../../../src/lib/canvas/resize';
import { elementBounds } from '../../../src/lib/canvas/geometry';
import type { CanvasBinding, CanvasElement, Point } from '../../../src/lib/canvas/model';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const shape = (id: string, x: number, y: number, width = 100, height = 60, type: CanvasElement['type'] = 'rectangle'): CanvasElement =>
  ({ id, type, x, y, width, height, ...base });
const elbow = (start?: CanvasBinding, end?: CanvasBinding, a: Point = { x: 0, y: 0 }, b: Point = { x: 1, y: 1 }, extra: Partial<CanvasElement> = {}): CanvasElement => ({
  id: 'a', type: 'arrow', x: a.x, y: a.y, width: b.x - a.x, height: b.y - a.y, points: [{ x: 0, y: 0 }, { x: b.x - a.x, y: b.y - a.y }],
  ...base, arrowType: 'elbow', ...(start ? { startBinding: start } : {}), ...(end ? { endBinding: end } : {}), ...extra,
});
const center = (elementId: string): CanvasBinding => ({ elementId, mode: 'center' });
const edge = (elementId: string, x: number, y: number): CanvasBinding => ({ elementId, mode: 'edge', anchor: { x, y } });
const lookupOf = (elements: readonly CanvasElement[]) => {
  const index = new Map(elements.map(el => [el.id, el]));
  return (id: string) => index.get(id);
};
const routed = (elements: readonly CanvasElement[], arrow: CanvasElement) => routeElbow(arrow, lookupOf(elements));
const route = (elements: readonly CanvasElement[], arrow: CanvasElement) => worldPoints(routed(elements, arrow));
const direction = (a: Point, b: Point) => vectorHeading(b.x - a.x, b.y - a.y);
const finite = (arrow: CanvasElement) => [arrow.x, arrow.y, arrow.width, arrow.height, ...arrow.points!.flatMap(p => [p.x, p.y])].every(Number.isFinite);

function expectOrthogonal(points: readonly Point[]) {
  for (let i = 1; i < points.length; i++) expect(points[i].x === points[i - 1].x || points[i].y === points[i - 1].y).toBe(true);
}

/** First and last segments follow the headings; no segment enters a target, and none but the stubs enters a padded box. */
function expectGoodRoute(elements: readonly CanvasElement[], arrow: CanvasElement) {
  const points = route(elements, arrow), [start, end] = elbowEnds(arrow, lookupOf(elements));
  expectOrthogonal(points);
  expect(points[0]).toEqual(start.point);
  expect(points[points.length - 1]).toEqual(end.point);
  expect(direction(points[0], points[1])).toBe(start.heading);
  expect(direction(points[points.length - 2], points[points.length - 1])).toBe(flip(end.heading));
  const targets = [start.box, end.box].filter(Boolean).map(b => boxOf(b!));
  const padded = routeObstacles(start, end).filter(Boolean);
  for (let i = 1; i < points.length; i++) {
    for (const box of targets) expect(segmentEntersBox(points[i - 1], points[i], box)).toBe(false);
    if (i > 1 && i < points.length - 1) for (const box of padded) expect(segmentEntersBox(points[i - 1], points[i], box!)).toBe(false);
  }
  return points;
}

const A = shape('A', 0, 0);

describe('elbow headings', () => {
  const ends = (elements: CanvasElement[], arrow: CanvasElement) => elbowEnds(arrow, lookupOf(elements));
  test('an edge binding heads out of the side its anchor is on, pulled out by the gap', () => {
    const [start, end] = ends([A, shape('B', 300, 200)], elbow(edge('A', 1, 0.25), edge('B', 0.5, 1)));
    expect(start).toMatchObject({ heading: RIGHT, point: { x: 105, y: 15 } });
    expect(end).toMatchObject({ heading: DOWN, point: { x: 350, y: 265 } });
    expect(ends([A], elbow(edge('A', 0, 0.5)))[0]).toMatchObject({ heading: LEFT, point: { x: -5, y: 30 } });
    expect(ends([A], elbow(edge('A', 0.3, 0)))[0]).toMatchObject({ heading: UP, point: { x: 30, y: -5 } });
  });
  test('the gap grows with the stroke', () => {
    expect(ends([A], elbow(edge('A', 1, 0.5), undefined, undefined, undefined, { strokeWidth: 8 }))[0].point).toEqual({ x: 108, y: 30 });
  });
  test('a centre binding leaves the midpoint of the side facing the other end', () => {
    const cases: [Point, Heading, Point][] = [
      [{ x: 400, y: 40 }, RIGHT, { x: 105, y: 30 }], [{ x: -300, y: 0 }, LEFT, { x: -5, y: 30 }],
      [{ x: 60, y: -300 }, UP, { x: 50, y: -5 }], [{ x: 0, y: 300 }, DOWN, { x: 50, y: 65 }],
      // Up-right of a wide box, the ray from the centre leaves through the top.
      [{ x: 200, y: -150 }, UP, { x: 50, y: -5 }],
    ];
    for (const [other, heading, point] of cases) expect(ends([A], elbow(center('A'), undefined, undefined, other))[0]).toEqual({ heading, point, box: elementBounds(A) });
  });
  test('a centre binding faces the other end’s anchor or centre when that end is bound', () => {
    const B = shape('B', 0, 300);
    expect(ends([A, B], elbow(center('A'), center('B')))[0].heading).toBe(DOWN);
    expect(ends([A, B], elbow(center('A'), edge('B', 1, 0.5)))[0].heading).toBe(DOWN);
    expect(ends([A, shape('C', 400, 0)], elbow(center('A'), edge('C', 0, 0.5)))[0].heading).toBe(RIGHT);
  });
  test('a centre end sharing its shape with an edge end takes the next side clockwise, so the two never meet', () => {
    expect(ends([A], elbow(center('A'), edge('A', 0.5, 1)))[0]).toMatchObject({ heading: LEFT, point: { x: -5, y: 30 } });
    expect(ends([A], elbow(edge('A', 1, 0.5), center('A')))[1]).toMatchObject({ heading: DOWN, point: { x: 50, y: 65 } });
  });
  test('an ellipse or diamond picks the side by the anchor’s dominant axis in the shape’s own proportions', () => {
    const ellipse = shape('E', 0, 0, 200, 40, 'ellipse'), diamond = shape('D', 0, 0, 100, 100, 'diamond');
    // The anchor sits 45 units right of and 18 above the centre: across a 200×40 ellipse that is its top.
    expect(ends([ellipse], elbow(edge('E', 0.75, 0)))[0].heading).toBe(UP);
    expect(ends([ellipse], elbow(edge('E', 1, 0.5)))[0]).toMatchObject({ heading: RIGHT, point: { x: 205, y: 20 } });
    expect(ends([diamond], elbow(edge('D', 0.2, 0.7)))[0].heading).toBe(LEFT);
    expect(ends([diamond], elbow(edge('D', 0.6, 0.9)))[0].heading).toBe(DOWN);
    expect(ends([diamond], elbow(center('D'), undefined, undefined, { x: 300, y: 80 }))[0]).toMatchObject({ heading: RIGHT, point: { x: 105, y: 50 } });
  });
  test('a free end keeps its point and heads along the dominant axis toward the other end', () => {
    const [start, end] = ends([], elbow(undefined, undefined, { x: 0, y: 0 }, { x: 100, y: 50 }));
    expect(start).toEqual({ point: { x: 0, y: 0 }, heading: RIGHT });
    expect(end).toEqual({ point: { x: 100, y: 50 }, heading: LEFT });
    const [free] = ends([shape('B', 0, 300)], elbow(undefined, center('B'), { x: 40, y: 0 }));
    expect(free.heading).toBe(DOWN);
  });
  test('a dangling binding leaves its end free where it is', () => {
    expect(ends([], elbow(center('gone'), undefined, { x: 5, y: 6 }, { x: 100, y: 6 }))[0]).toEqual({ point: { x: 5, y: 6 }, heading: RIGHT });
  });
});

describe('elbow routes', () => {
  test('side by side and stacked targets join in a straight line', () => {
    expect(expectGoodRoute([A, shape('B', 300, 0)], elbow(center('A'), center('B')))).toEqual([{ x: 105, y: 30 }, { x: 295, y: 30 }]);
    expect(expectGoodRoute([A, shape('B', 0, 200)], elbow(center('A'), center('B')))).toEqual([{ x: 50, y: 65 }, { x: 50, y: 195 }]);
  });
  test('offset and diagonal targets bend on the midline between them', () => {
    expect(expectGoodRoute([A, shape('B', 300, 100)], elbow(center('A'), center('B'))))
      .toEqual([{ x: 105, y: 30 }, { x: 200, y: 30 }, { x: 200, y: 130 }, { x: 295, y: 130 }]);
    expect(expectGoodRoute([A, shape('B', 300, 200)], elbow(center('A'), center('B'))))
      .toEqual([{ x: 50, y: 65 }, { x: 50, y: 130 }, { x: 350, y: 130 }, { x: 350, y: 195 }]);
    expect(expectGoodRoute([A, shape('B', 300, 200)], elbow(edge('A', 1, 0.5), edge('B', 0.5, 0))))
      .toEqual([{ x: 105, y: 30 }, { x: 350, y: 30 }, { x: 350, y: 195 }]);
  });
  test('ends facing away go round both targets, clear of their padding', () => {
    const points = expectGoodRoute([A, shape('B', 300, 0)], elbow(edge('A', 0, 0.5), edge('B', 1, 0.5)));
    expect(points).toEqual([{ x: -5, y: 30 }, { x: -40, y: 30 }, { x: -40, y: -40 }, { x: 440, y: -40 }, { x: 440, y: 30 }, { x: 405, y: 30 }]);
  });
  test('close targets, whose full padding would overlap, still get a route clear of both', () => {
    const B = shape('B', 130, 20);
    expect(expectGoodRoute([A, B], elbow(center('A'), center('B')))).toEqual([{ x: 105, y: 30 }, { x: 115, y: 30 }, { x: 115, y: 50 }, { x: 125, y: 50 }]);
    expectGoodRoute([A, B], elbow(edge('A', 0, 0.5), edge('B', 1, 0.5)));
    expectGoodRoute([A, B], elbow(edge('A', 0.5, 1), edge('B', 0.5, 0)));
  });
  test('both ends on the same shape go round it', () => {
    expect(expectGoodRoute([A], elbow(edge('A', 0.5, 0), edge('A', 1, 0.5))))
      .toEqual([{ x: 50, y: -5 }, { x: 50, y: -40 }, { x: 140, y: -40 }, { x: 140, y: 30 }, { x: 105, y: 30 }]);
    expect(expectGoodRoute([A], elbow(edge('A', 0.2, 0), edge('A', 0.8, 0))))
      .toEqual([{ x: 20, y: -5 }, { x: 20, y: -40 }, { x: 80, y: -40 }, { x: 80, y: -5 }]);
    expectGoodRoute([A], elbow(edge('A', 0, 0.5), edge('A', 1, 0.5)));
    expectGoodRoute([A], elbow(center('A'), edge('A', 0.5, 1)));
  });
  test('every side pairing in a ring of layouts is orthogonal, follows its headings and keeps clear', () => {
    const anchors = [{ x: 0.5, y: 0 }, { x: 1, y: 0.5 }, { x: 0.5, y: 1 }, { x: 0, y: 0.5 }, { x: 0.8, y: 1 }];
    for (const dx of [-300, -150, 0, 150, 300]) {
      for (const dy of [-200, -100, 0, 100, 200]) {
        if (!dx && !dy) continue;
        const B = shape('B', dx, dy), elements = [A, B];
        expectGoodRoute(elements, elbow(center('A'), center('B')));
        for (const s of anchors) {
          expectGoodRoute(elements, elbow(edge('A', s.x, s.y), center('B')));
          for (const e of anchors) expectGoodRoute(elements, elbow(edge('A', s.x, s.y), edge('B', e.x, e.y)));
        }
      }
    }
  });
  test('free ends join with a Z through the midpoint, leaving and entering along their headings', () => {
    expect(route([], elbow(undefined, undefined, { x: 0, y: 0 }, { x: 100, y: 50 }))).toEqual([{ x: 0, y: 0 }, { x: 50, y: 0 }, { x: 50, y: 50 }, { x: 100, y: 50 }]);
    expect(route([], elbow(undefined, undefined, { x: 0, y: 0 }, { x: 30, y: 100 }))).toEqual([{ x: 0, y: 0 }, { x: 0, y: 50 }, { x: 30, y: 50 }, { x: 30, y: 100 }]);
    expect(route([], elbow(undefined, undefined, { x: 0, y: 0 }, { x: 80, y: 0 }))).toEqual([{ x: 0, y: 0 }, { x: 80, y: 0 }]);
    expectGoodRoute([shape('B', 300, 200)], elbow(undefined, center('B'), { x: 0, y: 0 }));
    expectGoodRoute([A], elbow(center('A'), undefined, undefined, { x: 400, y: 300 }));
  });
  test('a free end close to its start target gets a route that stays clear of it', () => {
    expectGoodRoute([A], elbow(center('A'), undefined, undefined, { x: 120, y: 30 }));
    expectGoodRoute([A], elbow(edge('A', 1, 0.5), undefined, undefined, { x: 50, y: 90 }));
  });
  test('the fallback is an L across headings and a Z through the midpoint along them', () => {
    const end = (x: number, y: number, heading: Heading): RouteEnd => ({ point: { x, y }, heading });
    expect(fallbackRoute(end(0, 0, RIGHT), end(100, 60, UP))).toEqual([{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 60 }]);
    expect(fallbackRoute(end(0, 0, DOWN), end(100, 60, LEFT))).toEqual([{ x: 0, y: 0 }, { x: 0, y: 60 }, { x: 100, y: 60 }]);
    expect(fallbackRoute(end(0, 0, RIGHT), end(100, 60, LEFT))).toEqual([{ x: 0, y: 0 }, { x: 50, y: 0 }, { x: 50, y: 60 }, { x: 100, y: 60 }]);
    expect(fallbackRoute(end(0, 0, UP), end(100, 60, UP))).toEqual([{ x: 0, y: 0 }, { x: 0, y: 30 }, { x: 100, y: 30 }, { x: 100, y: 60 }]);
    expect(fallbackRoute(end(0, 0, RIGHT), end(100, 0, LEFT))).toEqual([{ x: 0, y: 0 }, { x: 100, y: 0 }]);
  });
  test('with no route clear of the targets, a free end inside the start target takes the fallback', () => {
    const [start, end] = elbowEnds(elbow(center('A'), undefined, undefined, { x: 50, y: 30 }), lookupOf([A]));
    const points = route([A], elbow(center('A'), undefined, undefined, { x: 50, y: 30 }));
    expect(points).toEqual(fallbackRoute(start, end));
    expectOrthogonal(points);
  });
  test('overlapping targets still get an orthogonal route between their sides', () => {
    for (const B of [shape('B', 50, 30), shape('B', 0, 0), shape('B', 10, 10, 20, 20)]) {
      for (const arrow of [elbow(center('A'), center('B')), elbow(edge('A', 1, 0.5), edge('B', 0, 0.5)), elbow(edge('A', 0.5, 0), center('B'))]) {
        const points = route([A, B], arrow), [start, end] = elbowEnds(arrow, lookupOf([A, B]));
        expectOrthogonal(points);
        expect(points[0]).toEqual(start.point);
        expect(points[points.length - 1]).toEqual(end.point);
      }
    }
  });
  test('routing is deterministic and returns the arrow itself when the route is unchanged', () => {
    const elements = [A, shape('B', 300, 100)], once = routed(elements, elbow(edge('A', 0.5, 0), edge('B', 0, 0.5)));
    expect(routed(elements, elbow(edge('A', 0.5, 0), edge('B', 0, 0.5)))).toEqual(once);
    expect(routed(elements, once)).toBe(once);
  });
  test('nothing is NaN for zero-size or inverted targets, coincident ends, or both centres on one shape', () => {
    const zero = shape('Z', 50, 50, 0, 0), flat = shape('F', 200, 0, 100, 0), inverted = shape('I', 300, 200, -100, -60);
    const arrows: [CanvasElement[], CanvasElement][] = [
      [[zero], elbow(center('Z'), edge('Z', 0.5, 0.5))],
      [[zero, flat], elbow(center('Z'), center('F'))],
      [[A, inverted], elbow(center('A'), edge('I', 1, 1))],
      [[A], elbow(center('A'), center('A'))],
      [[], elbow(undefined, undefined, { x: 7, y: 7 }, { x: 7, y: 7 })],
      [[A], elbow(center('A'), undefined, { x: 50, y: 30 }, { x: 50, y: 30 })],
      [[zero], elbow(undefined, center('Z'), { x: 50, y: 50 }, { x: 50, y: 50 })],
    ];
    for (const [elements, arrow] of arrows) {
      const out = routed(elements, arrow);
      expect(finite(out)).toBe(true);
      expect(out.points!.length).toBeGreaterThanOrEqual(2);
      expectOrthogonal(worldPoints(out));
    }
  });
  test('two free ends on the same point stay a two-point arrow', () => {
    expect(route([], elbow(undefined, undefined, { x: 3, y: 4 }, { x: 3, y: 4 }))).toEqual([{ x: 3, y: 4 }, { x: 3, y: 4 }]);
  });
  test('elbowRoute joins two RouteEnds directly', () => {
    expect(elbowRoute({ point: { x: 0, y: 0 }, heading: RIGHT }, { point: { x: 100, y: 0 }, heading: LEFT })).toEqual([{ x: 0, y: 0 }, { x: 100, y: 0 }]);
  });
});

describe('elbow arrows in the editor', () => {
  const B = shape('B', 300, 100);
  const elements = [A, B];
  const bound = routed(elements, elbow(center('A'), center('B')));
  const scene = [A, B, bound];

  test('drawing routes live: a free end and a bound end both get an elbow route', () => {
    const drawn = elbow(undefined, undefined, { x: 0, y: 0 }, { x: 0, y: 0 });
    const free = bindArrowEnd({ ...drawn, points: [{ x: 0, y: 0 }, { x: 100, y: 50 }], width: 100, height: 50 }, 'end', { x: 100, y: 50 }, [], 1).arrow;
    expect(worldPoints(free)).toEqual([{ x: 0, y: 0 }, { x: 50, y: 0 }, { x: 50, y: 50 }, { x: 100, y: 50 }]);
    const start = bindArrowEnd(drawn, 'start', { x: 50, y: 30 }, elements, 1);
    expect(start.arrow.startBinding).toEqual(center('A'));
    expect(finite(start.arrow)).toBe(true);
    const moving = { ...start.arrow, x: 50, y: 30, width: 300, height: 100, points: [{ x: 0, y: 0 }, { x: 300, y: 100 }] };
    const { arrow, snap } = bindArrowEnd(moving, 'end', { x: 350, y: 130 }, elements, 1);
    expect(snap?.target).toBe(B);
    expect(worldPoints(arrow)).toEqual(worldPoints(bound));
  });
  test('drawing a sharp arrow is not routed', () => {
    const sharp = { ...elbow(undefined, undefined, { x: 0, y: 0 }, { x: 100, y: 50 }), arrowType: 'sharp' as const };
    expect(worldPoints(bindArrowEnd(sharp, 'end', { x: 100, y: 50 }, [], 1).arrow)).toEqual([{ x: 0, y: 0 }, { x: 100, y: 50 }]);
  });
  test('an endpoint drag re-routes from the dragged end', () => {
    const last = bound.points!.length - 1;
    const dragged = bindArrowEnd(movePoint(bound, last, { x: 200, y: 400 }), 'end', { x: 200, y: 400 }, elements, 1).arrow;
    expect(dragged.endBinding).toBeUndefined();
    const points = worldPoints(dragged);
    expect(points[points.length - 1]).toEqual({ x: 200, y: 400 });
    expect(points[0]).toEqual({ x: 50, y: 65 });
    expectOrthogonal(points);
  });
  test('moving or resizing a target re-routes the whole arrow', () => {
    const moved = { ...B, x: 0, y: 300 };
    const followed = followBindings(scene, new Map([['B', moved]])).get('a')!;
    expect(worldPoints(followed)).toEqual(route([A, moved], bound));
    expect(worldPoints(followed)).toEqual([{ x: 50, y: 65 }, { x: 50, y: 295 }]);
    const grown = { ...B, width: 300, height: 300 };
    expect(worldPoints(followBindings(scene, new Map([['B', grown]])).get('a')!)).toEqual(route([A, grown], bound));
  });
  test('a change to the arrow itself re-routes it', () => {
    const thick = followBindings(scene, new Map([['a', { ...bound, strokeWidth: 10 }]])).get('a')!;
    expect(worldPoints(thick)[0]).toEqual({ x: 109, y: 30 });
  });
  test('moving the arrow with both targets keeps a valid route; moving it alone unbinds and keeps its points', () => {
    const together = followBindings(scene, moveElements(scene, new Set(['A', 'B', 'a']), 40, 25)).get('a')!;
    worldPoints(together).forEach((p, i) => { expect(p.x).toBeCloseTo(worldPoints(bound)[i].x + 40, 9); expect(p.y).toBeCloseTo(worldPoints(bound)[i].y + 25, 9); });
    expect(together.startBinding).toEqual(center('A'));
    const alone = followBindings(scene, moveElements(scene, new Set(['a']), 40, 25)).get('a')!;
    expect(alone.startBinding).toBeUndefined();
    expect(alone.endBinding).toBeUndefined();
    expect(worldPoints(alone)).toEqual(worldPoints(bound).map(p => ({ x: p.x + 40, y: p.y + 25 })));
  });
  test('switching to elbow routes the arrow; a bound one to its targets', () => {
    const sharp: CanvasElement = { ...elbow(center('A'), center('B'), { x: 50, y: 30 }, { x: 350, y: 130 }), arrowType: 'sharp' };
    const switched = stylePatchChanges([A, B, sharp], new Set(['a']), { arrowType: 'elbow' }).get('a')!;
    expect(switched.arrowType).toBe('elbow');
    expect(worldPoints(switched)).toEqual(worldPoints(bound));
    const zigzag: CanvasElement = { ...elbow(undefined, undefined, { x: 0, y: 0 }, { x: 100, y: 50 }), arrowType: 'curved', points: [{ x: 0, y: 0 }, { x: 70, y: -40 }, { x: 100, y: 50 }] };
    expect(worldPoints(stylePatchChanges([zigzag], new Set(['a']), { arrowType: 'elbow' }).get('a')!))
      .toEqual([{ x: 0, y: 0 }, { x: 50, y: 0 }, { x: 50, y: 50 }, { x: 100, y: 50 }]);
  });
  test('paste and duplicate route clones bound to cloned targets and keep the rest translated', () => {
    let n = 0;
    const clones = cloneCanvasElements(scene, { x: 24, y: 24 }, () => `c${n++}`);
    const changes = followBindings(scene, new Map(clones.map(el => [el.id, el])));
    const arrow = changes.get('c2')!;
    expect(arrow.startBinding).toEqual(center('c0'));
    expect(worldPoints(arrow)).toEqual(route(clones, arrow));
    const lone = cloneCanvasElements([bound], { x: 24, y: 24 }, () => 'x')[0];
    const kept = followBindings(scene, new Map([['x', lone]])).get('x')!;
    expect(kept.startBinding).toBeUndefined();
    expect(worldPoints(kept)).toEqual(worldPoints(bound).map(p => ({ x: p.x + 24, y: p.y + 24 })));
  });
  test('resizing a selection with an elbow arrow and its targets re-routes it', () => {
    const from = { x: 0, y: 0, width: 400, height: 160 }, to = { x: 0, y: 0, width: 800, height: 480 };
    const changes = followBindings(scene, resizeElements(scene, new Set(['A', 'B', 'a']), from, to));
    const arrow = changes.get('a')!;
    expect(worldPoints(arrow)).toEqual(route([changes.get('A')!, changes.get('B')!], arrow));
    expectOrthogonal(worldPoints(arrow));
  });
  test('removing a target drops its binding and re-routes from the other one', () => {
    const arrow = followBindings(scene, new Map([['B', null]])).get('a')!;
    expect(arrow.endBinding).toBeUndefined();
    expect(finite(arrow)).toBe(true);
    expectOrthogonal(worldPoints(arrow));
  });
});
