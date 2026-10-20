import { describe, expect, test } from 'bun:test';
import { catmullRomCubics, cubicAt, flattenCubics } from '../../../src/lib/canvas/arrow/curve';
import { arrowPath, linePath, pathEnd, polylineEnd } from '../../../src/lib/canvas/arrow/path';
import { elementArrowheads, elementBounds, elementHitTest, segmentDistance } from '../../../src/lib/canvas/geometry';
import { exportCanvasSvg } from '../../../src/lib/canvas/renderSvg';
import { createCanvasDocument, parseCanvasDocument, serializeCanvasDocument, type CanvasElement, type Point } from '../../../src/lib/canvas/model';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const arrow = (points: Point[], extra: Partial<CanvasElement> = {}): CanvasElement => ({
  id: 'a', type: 'arrow', x: 10, y: 20, width: points[points.length - 1].x, height: points[points.length - 1].y, points, ...base, ...extra,
});
const bend = [{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 100 }];
const wave = [{ x: 0, y: 0 }, { x: 60, y: -40 }, { x: 120, y: 30 }, { x: 200, y: 0 }];

describe('curved arrow path', () => {
  test('flattening keeps the ends exact and passes through every point', () => {
    const points = linePath(arrow(wave, { arrowType: 'curved' })).polyline;
    expect(points[0]).toEqual(wave[0]);
    expect(points[points.length - 1]).toEqual(wave[3]);
    for (const p of wave) expect(points).toContainEqual(p);
    expect(points.length).toBeGreaterThan(wave.length * 4);
  });
  test('the polyline stays within a quarter unit of the curve', () => {
    const cubics = catmullRomCubics(wave), polyline = flattenCubics(wave[0], cubics, 0.25);
    let from = wave[0];
    for (const cubic of cubics) {
      for (let t = 0; t <= 1; t += 1 / 200) {
        const p = cubicAt(from, cubic, t);
        let near = Infinity;
        for (let i = 1; i < polyline.length; i++) near = Math.min(near, segmentDistance(p, polyline[i - 1], polyline[i]));
        expect(near).toBeLessThan(0.25);
      }
      from = cubic.to;
    }
  });
  test('two points stay straight, and sharp and elbow arrows are their points', () => {
    const two = [{ x: 0, y: 0 }, { x: 100, y: 40 }];
    expect(linePath(arrow(two, { arrowType: 'curved' })).curves).toEqual([]);
    expect(arrowPath(arrow(two, { arrowType: 'curved' }))).toEqual(two);
    expect(arrowPath(arrow(bend))).toEqual(bend);
    expect(arrowPath(arrow(bend, { arrowType: 'elbow' }))).toEqual(bend);
    expect(linePath({ ...arrow(bend, { arrowType: 'curved' }), type: 'line' }).curves).toEqual([]);
  });
  test('bounds include the curve bulging past its points', () => {
    expect(elementBounds(arrow(bend))).toEqual({ x: 10, y: 20, width: 100, height: 100 });
    const curved = elementBounds(arrow(bend, { arrowType: 'curved' }));
    expect(curved.x).toBe(10);
    expect(curved.y).toBeLessThan(20 - 1);
    expect(curved.x + curved.width).toBeGreaterThan(110 + 1);
    expect(curved.y + curved.height).toBe(120);
  });
  test('hit testing follows the curve, not the straight segments', () => {
    const curved = arrow(bend, { arrowType: 'curved', endArrowhead: 'none' });
    const outside = { x: 10 + 50, y: 20 + 50 };
    expect(elementHitTest(arrow(bend), { x: 10 + 100, y: 20 + 50 }, 1)).toBeTrue();
    expect(elementHitTest(curved, outside, 1)).toBeFalse();
    const onCurve = cubicAt(bend[1], linePath(curved).curves[1], 0.5);
    expect(elementHitTest(curved, { x: 10 + onCurve.x, y: 20 + onCurve.y }, 1)).toBeTrue();
    expect(elementHitTest(arrow(bend, { endArrowhead: 'none' }), { x: 10 + onCurve.x, y: 20 + onCurve.y }, 1)).toBeFalse();
  });
});

describe('arrowheads on a path', () => {
  test('a curve aims its head along its end tangent, not its last flattened segment', () => {
    const curved = arrow(bend, { arrowType: 'curved' });
    expect(pathEnd(curved, 'end').angle).toBeCloseTo(Math.PI / 2, 12);
    expect(pathEnd(curved, 'start').angle).toBeCloseTo(Math.PI, 12);
    expect(polylineEnd(linePath(curved).polyline, 'end').angle).not.toBeCloseTo(Math.PI / 2, 3);
    const [head] = elementArrowheads(curved);
    expect(head.tip).toEqual({ x: 100, y: 100 });
    expect(head.wings[0].y).toBeCloseTo(head.wings[1].y, 9);
  });
  test('size comes from the whole path, so a short last segment keeps a full head', () => {
    const elbow = arrow([{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 6 }], { arrowType: 'elbow' });
    expect(elementArrowheads(elbow)[0].size).toBe(16);
    expect(elementArrowheads(arrow([{ x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 20 }]))[0].size).toBe(10);
  });
  test('coincident points do not turn a head', () => {
    const doubled = arrow([{ x: 0, y: 0 }, { x: 50, y: 50 }, { x: 50, y: 50 }]);
    expect(pathEnd(doubled, 'end').angle).toBeCloseTo(Math.PI / 4, 12);
  });
});

describe('arrow type in the format and export', () => {
  test('is validated and round-trips', () => {
    const doc = { ...createCanvasDocument(), elements: ['sharp', 'curved', 'elbow'].map((arrowType, i) => arrow(bend, { id: String(i), arrowType } as Partial<CanvasElement>)) };
    expect(parseCanvasDocument(serializeCanvasDocument(doc))).toEqual(doc);
    expect(() => parseCanvasDocument(JSON.stringify({ ...doc, elements: [arrow(bend, { arrowType: 'round' } as never)] }))).toThrow('invalid arrowType');
  });
  test('SVG draws a curved arrow with cubic commands and a sharp one with lines', () => {
    const svg = (el: CanvasElement) => exportCanvasSvg({ ...createCanvasDocument(), elements: [el] }).match(/<path d="([^"]+)"/)![1];
    expect(svg(arrow(wave, { arrowType: 'curved' })).match(/ C /g)).toHaveLength(3);
    expect(svg(arrow(wave, { arrowType: 'curved' }))).not.toContain(' L ');
    expect(svg(arrow(wave))).toBe('M 0 0 L 60 -40 L 120 30 L 200 0');
    expect(svg(arrow(bend, { arrowType: 'elbow' }))).toBe('M 0 0 L 100 0 L 100 100');
  });
});
