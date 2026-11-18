import { describe, expect, test } from 'bun:test';
import { routeElbow } from '../../../src/lib/canvas/elbow';
import { moveSegment, releaseSegment } from '../../../src/lib/canvas/elbow/segments';
import { elementLanes, runsAlong, type Lane } from '../../../src/lib/canvas/elbow/lanes';
import { pointHandleAt } from '../../../src/lib/canvas/arrow/handles';
import { worldPoints } from '../../../src/lib/canvas/arrow/points';
import { bindArrowEnd, followBindings, moveElements } from '../../../src/lib/canvas/bindings';
import { stylePatchChanges } from '../../../src/lib/canvas/labels';
import { cloneCanvasElements } from '../../../src/lib/canvas/renderClipboard';
import { resizeElements } from '../../../src/lib/canvas/resize';
import { selectionFrame } from '../../../src/lib/canvas/render';
import { createCanvasDocument, parseCanvasDocument, serializeCanvasDocument, type CanvasBinding, type CanvasElement, type Point } from '../../../src/lib/canvas/model';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const shape = (id: string, x: number, y: number, width = 100, height = 60): CanvasElement => ({ id, type: 'rectangle', x, y, width, height, ...base });
const elbow = (start?: CanvasBinding, end?: CanvasBinding, a: Point = { x: 0, y: 0 }, b: Point = { x: 1, y: 1 }): CanvasElement => ({
  id: 'a', type: 'arrow', x: a.x, y: a.y, width: b.x - a.x, height: b.y - a.y, points: [{ x: 0, y: 0 }, { x: b.x - a.x, y: b.y - a.y }],
  ...base, arrowType: 'elbow', ...(start ? { startBinding: start } : {}), ...(end ? { endBinding: end } : {}),
});
const center = (elementId: string): CanvasBinding => ({ elementId, mode: 'center' });
const edge = (elementId: string, x: number, y: number): CanvasBinding => ({ elementId, mode: 'edge', anchor: { x, y } });
const lookupOf = (elements: readonly CanvasElement[]) => {
  const index = new Map(elements.map(el => [el.id, el]));
  return (id: string) => index.get(id);
};
const finite = (arrow: CanvasElement) => [arrow.x, arrow.y, arrow.width, arrow.height, ...arrow.points!.flatMap(p => [p.x, p.y])].every(Number.isFinite);
const runsOn = (points: readonly Point[], lane: Lane) => points.slice(1).some((p, i) => runsAlong(lane, points[i], p));
function expectOrthogonal(points: readonly Point[]) {
  for (let i = 1; i < points.length; i++) expect(points[i].x === points[i - 1].x || points[i].y === points[i - 1].y).toBe(true);
}

const A = shape('A', 0, 0), B = shape('B', 300, 100), elements = [A, B], lookup = lookupOf(elements);
// [(105,30), (200,30), (200,130), (295,130)]: out of A's right side, down the midline, into B's left side.
const z = routeElbow(elbow(center('A'), center('B')), lookup);
// The middle segment dragged 30 left, to x = 170.
const dragged = moveSegment(z, 1, { x: -30, y: 999 }, lookup);
const lane: Lane = { horizontal: false, at: 170 };

describe('dragging an elbow segment', () => {
  test('moves only that segment, perpendicular to itself, and fixes its line', () => {
    expect(worldPoints(z)).toEqual([{ x: 105, y: 30 }, { x: 200, y: 30 }, { x: 200, y: 130 }, { x: 295, y: 130 }]);
    expect(worldPoints(dragged)).toEqual([{ x: 105, y: 30 }, { x: 170, y: 30 }, { x: 170, y: 130 }, { x: 295, y: 130 }]);
    expect(dragged.fixedSegments).toEqual([{ x: 65 }]);
    expect(elementLanes(dragged)).toEqual([lane]);
    const horizontal = moveSegment(z, 0, { x: 999, y: 0 }, lookup);
    expect(worldPoints(horizontal)).toEqual(worldPoints(z));
  });
  test('dragging the same segment again replaces its fixed line instead of adding one', () => {
    const again = moveSegment(dragged, 1, { x: 50, y: 0 }, lookup);
    expect(worldPoints(again)).toEqual([{ x: 105, y: 30 }, { x: 220, y: 30 }, { x: 220, y: 130 }, { x: 295, y: 130 }]);
    expect(elementLanes(again)).toEqual([{ horizontal: false, at: 220 }]);
  });
  test('a dragged route is kept as it is while its ends stay put', () => {
    expect(routeElbow(dragged, lookup)).toBe(dragged);
    expect(followBindings([A, B, dragged], new Map([['a', dragged]])).get('a')).toBe(dragged);
  });
  test('the first segment keeps leaving its target along the heading, by a stub', () => {
    const l = routeElbow(elbow(edge('A', 1, 0.5), edge('B', 0.5, 0)), lookup);
    expect(worldPoints(l)).toEqual([{ x: 105, y: 30 }, { x: 350, y: 30 }, { x: 350, y: 95 }]);
    const first = moveSegment(l, 0, { x: 0, y: 20 }, lookup);
    expect(worldPoints(first)).toEqual([{ x: 105, y: 30 }, { x: 145, y: 30 }, { x: 145, y: 50 }, { x: 350, y: 50 }, { x: 350, y: 95 }]);
    expect(elementLanes(first)).toEqual([{ horizontal: true, at: 50 }]);
  });
  test('the last segment keeps entering its target along the heading, and a short one gets a half-length stub', () => {
    const l = routeElbow(elbow(edge('A', 1, 0.5), edge('B', 0.5, 0)), lookup);
    const last = moveSegment(l, 1, { x: 30, y: 0 }, lookup);
    // The last segment is 65 long, so the stub is the full 40.
    expect(worldPoints(last)).toEqual([{ x: 105, y: 30 }, { x: 380, y: 30 }, { x: 380, y: 55 }, { x: 350, y: 55 }, { x: 350, y: 95 }]);
    const closer = [A, shape('B', 300, 70)], near = routeElbow(elbow(edge('A', 1, 0.5), edge('B', 0.5, 0)), lookupOf(closer));
    expect(worldPoints(near)).toEqual([{ x: 105, y: 30 }, { x: 350, y: 30 }, { x: 350, y: 65 }]);
    expect(worldPoints(moveSegment(near, 1, { x: 30, y: 0 }, lookupOf(closer))))
      .toEqual([{ x: 105, y: 30 }, { x: 380, y: 30 }, { x: 380, y: 47.5 }, { x: 350, y: 47.5 }, { x: 350, y: 65 }]);
  });
  test('a single straight segment becomes a detour with a stub at each end', () => {
    const level = [A, shape('B', 300, 0)], straight = routeElbow(elbow(center('A'), center('B')), lookupOf(level));
    expect(worldPoints(moveSegment(straight, 0, { x: 0, y: 50 }, lookupOf(level))))
      .toEqual([{ x: 105, y: 30 }, { x: 145, y: 30 }, { x: 145, y: 80 }, { x: 255, y: 80 }, { x: 255, y: 30 }, { x: 295, y: 30 }]);
  });
  test('a free end turns straight onto its dragged first segment', () => {
    const free = routeElbow(elbow(undefined, undefined, { x: 0, y: 0 }, { x: 100, y: 50 }), lookupOf([]));
    expect(worldPoints(moveSegment(free, 0, { x: 0, y: 20 }, lookupOf([]))))
      .toEqual([{ x: 0, y: 0 }, { x: 0, y: 20 }, { x: 50, y: 20 }, { x: 50, y: 50 }, { x: 100, y: 50 }]);
  });
  test('dragged past an end, the route still runs along the line and leaves and enters along the headings', () => {
    const u = routeElbow(elbow(edge('A', 0.5, 0), edge('B', 0.5, 0)), lookup);
    expect(worldPoints(u)).toEqual([{ x: 50, y: -5 }, { x: 50, y: -40 }, { x: 350, y: -40 }, { x: 350, y: 95 }]);
    const points = worldPoints(moveSegment(u, 1, { x: 0, y: 90 }, lookup));
    expectOrthogonal(points);
    expect(runsOn(points, { horizontal: true, at: 50 })).toBe(true);
    expect(points[1]).toEqual({ x: 50, y: -40 });
    expect(points.at(-2)!.x).toBe(350);
    expect(points.at(-2)!.y).toBeLessThan(95);
  });
});

describe('fixed segments when the ends move', () => {
  test('a moved target re-routes the rest and keeps the fixed line', () => {
    const right = followBindings([A, B, dragged], new Map([['B', shape('B', 400, 100)]])).get('a')!;
    expect(worldPoints(right)).toEqual([{ x: 105, y: 30 }, { x: 170, y: 30 }, { x: 170, y: 130 }, { x: 395, y: 130 }]);
    expect(elementLanes(right)).toEqual([lane]);
    const lower = followBindings([A, B, dragged], new Map([['B', shape('B', 300, 200)]])).get('a')!;
    expect(worldPoints(lower)).toEqual([{ x: 105, y: 30 }, { x: 170, y: 30 }, { x: 170, y: 230 }, { x: 295, y: 230 }]);
  });
  test('a centre-bound end leaves by the side that keeps the fixed line in a short route', () => {
    // B straight below A would face A's bottom; the fixed vertical at x = 170 makes the right side the cheaper one.
    const below = followBindings([A, B, dragged], new Map([['B', shape('B', 120, 300)]])).get('a')!;
    const points = worldPoints(below);
    expectOrthogonal(points);
    expect(runsOn(points, lane)).toBe(true);
    expect(points[0]).toEqual({ x: 105, y: 30 });
    // Its first segment's stub follows the side the route took, not the side facing B.
    const scene = [A, shape('B', 120, 300)], first = worldPoints(moveSegment(below, 0, { x: 0, y: -20 }, lookupOf(scene)));
    expect(first.slice(0, 3)).toEqual([{ x: 105, y: 30 }, { x: 145, y: 30 }, { x: 145, y: 10 }]);
  });
  test('when the ends come level the fixed segment shrinks away rather than making a detour, and returns after', () => {
    const level = followBindings([A, B, dragged], new Map([['B', shape('B', 300, 0)]])).get('a')!;
    expect(worldPoints(level)).toEqual([{ x: 105, y: 30 }, { x: 295, y: 30 }]);
    expect(elementLanes(level)).toEqual([lane]);
    const back = followBindings([A, shape('B', 300, 0), level], new Map([['B', B]])).get('a')!;
    expect(worldPoints(back)).toEqual(worldPoints(dragged));
  });
  test('the arrow moved alone carries its fixed segments; moved with its targets it keeps its route', () => {
    const scene = [A, B, dragged];
    const alone = followBindings(scene, moveElements(scene, new Set(['a']), 40, 25)).get('a')!;
    expect(alone.fixedSegments).toEqual(dragged.fixedSegments);
    expect(elementLanes(alone)).toEqual([{ horizontal: false, at: 210 }]);
    expect(worldPoints(alone)).toEqual(worldPoints(dragged).map(p => ({ x: p.x + 40, y: p.y + 25 })));
    const together = followBindings(scene, moveElements(scene, new Set(['A', 'B', 'a']), 40, 25)).get('a')!;
    expect(worldPoints(together)).toEqual(worldPoints(dragged).map(p => ({ x: p.x + 40, y: p.y + 25 })));
    expect(elementLanes(together)).toEqual([{ horizontal: false, at: 210 }]);
  });
  test('an endpoint drag keeps the fixed segments where they are in the world', () => {
    const last = dragged.points!.length - 1;
    const free = bindArrowEnd({ ...dragged, points: dragged.points!.map((p, i) => i === last ? { x: 395, y: 200 } : p) }, 'end', { x: 500, y: 230 }, elements, 1).arrow;
    const points = worldPoints(free);
    expect(free.endBinding).toBeUndefined();
    expect(elementLanes(free)).toEqual([lane]);
    expect(runsOn(points, lane)).toBe(true);
    expectOrthogonal(points);
  });
  test('paste and resize carry fixed segments with the arrow', () => {
    const [clone] = cloneCanvasElements([dragged], { x: 24, y: 24 }, () => 'c');
    expect(elementLanes(clone)).toEqual([{ horizontal: false, at: 194 }]);
    const resized = resizeElements([dragged], new Set(['a']), { x: 105, y: 30, width: 190, height: 100 }, { x: 105, y: 30, width: 380, height: 100 }).get('a')!;
    expect(elementLanes(resized)).toEqual([{ horizontal: false, at: 235 }]);
  });
  test('nothing is NaN with fixed segments on zero-size targets, coincident ends or far lanes', () => {
    const zero = shape('Z', 50, 50, 0, 0);
    const cases: [CanvasElement[], CanvasElement][] = [
      [[zero, B], { ...elbow(center('Z'), center('B')), fixedSegments: [{ x: 10 }, { y: -400 }] }],
      [[A], { ...elbow(center('A'), edge('A', 1, 0.5)), fixedSegments: [{ y: 1e5 }] }],
      [[], { ...elbow(undefined, undefined, { x: 5, y: 5 }, { x: 5, y: 5 }), fixedSegments: [{ x: 3 }] }],
      [[], { ...elbow(undefined, undefined, { x: 0, y: 0 }, { x: 0, y: 80 }), fixedSegments: [{ y: 40 }, { y: 40 }] }],
      [[A, shape('B', 50, 30)], { ...elbow(center('A'), center('B')), fixedSegments: [{ x: 0 }] }],
    ];
    for (const [scene, arrow] of cases) {
      const out = routeElbow(arrow, lookupOf(scene));
      expect(finite(out)).toBe(true);
      expect(out.points!.length).toBeGreaterThanOrEqual(2);
      expectOrthogonal(worldPoints(out));
    }
  });
});

describe('clearing fixed segments', () => {
  test('releasing a fixed segment routes the arrow automatically; a free segment releases nothing', () => {
    const released = releaseSegment(dragged, 1, lookup);
    expect(released.fixedSegments).toBeUndefined();
    expect(worldPoints(released)).toEqual(worldPoints(z));
    expect(releaseSegment(dragged, 0, lookup)).toBe(dragged);
  });
  test('releasing one of two keeps the other', () => {
    const l = routeElbow(elbow(edge('A', 1, 0.5), edge('B', 0.5, 0)), lookup);
    const two = moveSegment(moveSegment(l, 0, { x: 0, y: 20 }, lookup), 3, { x: 30, y: 0 }, lookup);
    expect(elementLanes(two)).toEqual([{ horizontal: true, at: 50 }, { horizontal: false, at: 380 }]);
    const points = worldPoints(two), index = points.findIndex((p, i) => i > 0 && runsAlong({ horizontal: false, at: 380 }, points[i - 1], p)) - 1;
    expect(elementLanes(releaseSegment(two, index, lookup))).toEqual([{ horizontal: true, at: 50 }]);
  });
  test('changing the arrow type drops them; restyling an elbow arrow keeps them', () => {
    expect(stylePatchChanges([A, B, dragged], new Set(['a']), { arrowType: 'sharp' }).get('a')!.fixedSegments).toBeUndefined();
    expect(stylePatchChanges([A, B, dragged], new Set(['a']), { arrowType: 'curved' }).get('a')!.fixedSegments).toBeUndefined();
    expect(stylePatchChanges([A, B, dragged], new Set(['a']), { strokeStyle: 'dashed' }).get('a')!.fixedSegments).toEqual(dragged.fixedSegments);
  });
  test('binding an end to a different shape drops them; the same shape or no shape keeps them', () => {
    const C = shape('C', 300, 300), scene = [A, B, C];
    const rebound = bindArrowEnd(dragged, 'end', { x: 350, y: 330 }, scene, 1).arrow;
    expect(rebound.endBinding?.elementId).toBe('C');
    expect(rebound.fixedSegments).toBeUndefined();
    expect(worldPoints(rebound)).toEqual(worldPoints(routeElbow({ ...dragged, fixedSegments: undefined, endBinding: center('C') }, lookupOf(scene))));
    const same = bindArrowEnd(dragged, 'end', { x: 300, y: 130 }, scene, 1).arrow;
    expect(same.endBinding?.elementId).toBe('B');
    expect(same.fixedSegments).toEqual(expect.any(Array));
    expect(bindArrowEnd(dragged, 'end', { x: 600, y: 600 }, scene, 1).arrow.fixedSegments).toEqual(expect.any(Array));
  });
});

describe('fixed segments in the format', () => {
  const arrow = { ...elbow(), id: 'arrow', fixedSegments: [{ x: 12.5 }, { y: -40 }] };
  test('round-trips as an optional field', () => {
    const doc = { ...createCanvasDocument(), elements: [arrow] };
    expect(parseCanvasDocument(serializeCanvasDocument(doc))).toEqual(doc);
  });
  test.each([
    ['not an array', { x: 1 }], ['an empty entry', [{}]], ['both axes', [{ x: 1, y: 2 }]], ['a string', [{ x: '1' }]],
    ['an unknown key', [{ z: 1 }]], ['null', [null]], ['a non-finite value', [{ y: null }]],
  ])('refuses %s', (_, value) => {
    expect(() => parseCanvasDocument(JSON.stringify({ ...createCanvasDocument(), elements: [{ ...arrow, fixedSegments: value }] }))).toThrow('invalid fixed segments');
  });
});

describe('a selected line or arrow', () => {
  const line: CanvasElement = { ...elbow(), id: 'l', type: 'line', arrowType: undefined };
  const stroke: CanvasElement = { ...line, id: 'f', type: 'freedraw' };
  test('has no selection frame on its own; a group with it, a stroke or a shape does', () => {
    expect(selectionFrame([dragged])).toBeNull();
    expect(selectionFrame([line])).toBeNull();
    expect(selectionFrame([dragged, line])).not.toBeNull();
    expect(selectionFrame([dragged, A])).not.toBeNull();
    expect(selectionFrame([stroke])).not.toBeNull();
    expect(selectionFrame([A])).toEqual({ x: 0, y: 0, width: 100, height: 60 });
  });
  test('an elbow arrow’s segment handle is found at its segment’s middle; its ends win near them', () => {
    expect(pointHandleAt(dragged, { x: 171, y: 82 }, 1)).toMatchObject({ kind: 'segment', segment: 1 });
    expect(pointHandleAt(dragged, { x: 106, y: 31 }, 1)).toMatchObject({ kind: 'point', index: 0 });
  });
});
