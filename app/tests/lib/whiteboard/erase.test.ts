import { describe, expect, test } from 'bun:test';
import { applyElementChanges } from '../../../src/lib/canvas/changes';
import { eraseStroke, sweepTouches } from '../../../src/lib/canvas/erase';
import { parseCanvasDocument, serializeCanvasDocument, createCanvasDocument, type CanvasElement } from '../../../src/lib/canvas/model';

// A horizontal stroke from world (10, 20) to (110, 20) through (60, 20). Subdivided at radius / 4 = 1.25 units, so with
// a radius of 5 and a width of 2 (reach 6) a dab at x = 60 takes the points from 54.x to 65.x.
const stroke: CanvasElement = {
  id: 's', type: 'freedraw', x: 10, y: 20, width: 100, height: 0, stroke: '#123456', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 7,
  opacity: 0.8, custom: 'kept', points: [{ x: 0, y: 0 }, { x: 50, y: 0 }, { x: 100, y: 0 }],
};
const ids = () => { let n = 0; return () => `n${++n}`; };
const world = (element: CanvasElement) => element.points!.map(p => ({ x: element.x + p.x, y: element.y + p.y }));

describe('partial erasing', () => {
  test('a dab cuts a stroke in two: the first piece keeps its ID, the second gets a new one', () => {
    const result = eraseStroke(stroke, [{ x: 60, y: 20 }], 5, ids())!;
    expect(result.kept.map(piece => piece.id)).toEqual(['s', 'n1']);
    const [left, right] = result.kept;
    expect(left).toMatchObject({ x: 10, y: 20, width: 43.75, height: 0, points: [{ x: 0, y: 0 }, { x: 43.75, y: 0 }] });
    expect(right).toMatchObject({ x: 66.25, y: 20, width: 43.75, points: [{ x: 0, y: 0 }, { x: 43.75, y: 0 }] });
    // Everything else about the stroke survives the cut.
    expect(right).toMatchObject({ type: 'freedraw', stroke: '#123456', strokeWidth: 2, seed: 7, opacity: 0.8, custom: 'kept' });
    expect(result.removed).toHaveLength(1);
    expect(world(result.removed[0]).map(p => p.x)).toEqual([53.75, 60, 66.25]);
  });
  test('the cut lands within a subdivision step of the eraser edge', () => {
    const [left, right] = eraseStroke(stroke, [{ x: 60, y: 20 }], 5, ids())!.kept;
    const leftEnd = world(left).at(-1)!.x, rightStart = world(right)[0].x;
    expect(60 - 6 - leftEnd).toBeGreaterThan(0);
    expect(60 - 6 - leftEnd).toBeLessThanOrEqual(1.25);
    expect(rightStart - (60 + 6)).toBeLessThanOrEqual(1.25);
  });
  test('several cuts leave several pieces in stroke order', () => {
    const result = eraseStroke(stroke, [{ x: 40, y: 20 }, { x: 40, y: 20 }], 5, ids())!;
    const again = eraseStroke(stroke, [{ x: 40, y: 20 }, { x: 40, y: 50 }, { x: 80, y: 50 }, { x: 80, y: 20 }], 5, ids())!;
    expect(result.kept).toHaveLength(2);
    expect(again.kept.map(piece => piece.id)).toEqual(['s', 'n1', 'n2']);
    const starts = again.kept.map(piece => piece.x);
    expect(starts).toEqual([...starts].sort((a, b) => a - b));
    expect(again.removed).toHaveLength(2);
  });
  test('a stroke the eraser covers entirely leaves nothing', () => {
    const result = eraseStroke(stroke, [{ x: 0, y: 20 }, { x: 120, y: 20 }], 5, ids())!;
    expect(result.kept).toEqual([]);
    expect(result.removed).toHaveLength(1);
  });
  test('pieces too short to draw are dropped', () => {
    // A dab at 100 leaves a 3.75-unit tail past 106.
    expect(eraseStroke(stroke, [{ x: 100, y: 20 }], 5, ids())!.kept).toHaveLength(2);
    expect(eraseStroke(stroke, [{ x: 100, y: 20 }], 5, ids(), { minLength: 5 })!.kept.map(piece => piece.id)).toEqual(['s']);
    // Half the stroke's width is the floor whatever the caller asks.
    expect(eraseStroke({ ...stroke, strokeWidth: 10 }, [{ x: 100, y: 20 }], 1, ids())!.kept.map(piece => piece.id)).toEqual(['s']);
  });
  test('the swept segment between samples cuts, though neither sample reaches the stroke', () => {
    expect(eraseStroke(stroke, [{ x: 30, y: -10 }], 5, ids())).toBeNull();
    expect(eraseStroke(stroke, [{ x: 30, y: 50 }], 5, ids())).toBeNull();
    const swept = eraseStroke(stroke, [{ x: 30, y: -10 }, { x: 30, y: 50 }], 5, ids())!;
    expect(swept.kept.map(piece => piece.id)).toEqual(['s', 'n1']);
  });
  test('a clip keeps the part outside it', () => {
    expect(eraseStroke(stroke, [{ x: 60, y: 20 }], 5, ids(), { clip: { x: 0, y: 0, width: 50, height: 100 } })).toBeNull();
  });
  test('pieces are valid elements that save', () => {
    const { kept } = eraseStroke(stroke, [{ x: 60, y: 20 }], 5, ids())!;
    const document = { ...createCanvasDocument(), elements: kept };
    expect(parseCanvasDocument(serializeCanvasDocument(document)).elements).toEqual(kept);
  });
  test('pieces added with the stroke commit directly above it', () => {
    const below = { ...stroke, id: 'below' }, above = { ...stroke, id: 'above' };
    const [first, second] = eraseStroke(stroke, [{ x: 60, y: 20 }], 5, ids())!.kept;
    const next = applyElementChanges([below, stroke, above], new Map([[first.id, first], [second.id, second]]), new Map([['s', [second.id]]]));
    expect(next.map(element => element.id)).toEqual(['below', 's', 'n1', 'above']);
  });
  test('a shape is touched anywhere along the sweep', () => {
    const box: CanvasElement = { ...stroke, id: 'b', type: 'rectangle', x: 0, y: 0, width: 20, height: 20, points: undefined };
    expect(sweepTouches(box, { x: -30, y: 10 }, { x: -12, y: 10 }, 5)).toBe(false);
    expect(sweepTouches(box, { x: -30, y: 10 }, { x: 50, y: 10 }, 5)).toBe(true);
  });
});
