import { describe, expect, test } from 'bun:test';
import { LiveStroke, type LiveStrokeContext } from '@/lib/whiteboard/liveStroke';
import type { WhiteboardElement, Point } from '@/lib/whiteboard/model';

type Call = [name: string, ...args: number[]];
function fakeContext() {
  const calls: Call[] = [];
  const record = (name: string) => (...args: number[]) => { calls.push([name, ...args]); };
  const context = {
    canvas: { width: 400, height: 300 }, strokeStyle: '', lineWidth: 0, lineCap: 'butt', lineJoin: 'miter',
    setTransform: record('setTransform'), clearRect: record('clearRect'), beginPath: record('beginPath'), moveTo: record('moveTo'),
    lineTo: record('lineTo'), quadraticCurveTo: record('quadraticCurveTo'), stroke: record('stroke'),
  };
  return { context: context as unknown as LiveStrokeContext, calls, named: (name: string) => calls.filter(call => call[0] === name) };
}
const element: WhiteboardElement = { id: 'ink', type: 'freedraw', x: 100, y: 50, width: 0, height: 0, stroke: '#123456', fill: 'transparent', strokeWidth: 4, style: 'clean', seed: 1 };
const viewport = { x: 10, y: 20, zoom: 2 };
const points: Point[] = [{ x: 0, y: 0 }, { x: 0, y: 0 }, { x: 10, y: 0 }, { x: 20, y: 10 }, { x: 20, y: 30 }, { x: 0, y: 40 }];
const at = (path: Point[]): WhiteboardElement => ({ ...element, points: path });
function setup() {
  const ink = fakeContext(), tail = fakeContext();
  const stroke = new LiveStroke(ink.context, tail.context);
  stroke.begin(element, viewport, 2);
  return { ink, tail, stroke };
}
/** The curve pieces `elementPaths` in `render.ts` gives a freedraw element, as quadraticCurveTo arguments. */
const referenceCurves = (path: Point[]) => path.slice(1, -1).map((bend, i) => [bend.x, bend.y, (bend.x + path[i + 2].x) / 2, (bend.y + path[i + 2].y) / 2]);

describe('live stroke', () => {
  test('places the stroke where the scene would paint it and takes its look from the element', () => {
    const { ink, tail } = setup();
    for (const layer of [ink, tail]) {
      expect(layer.calls).toEqual([['setTransform', 4, 0, 0, 4, 2 * (10 + 2 * 100), 2 * (20 + 2 * 50)]]);
      expect(layer.context).toMatchObject({ strokeStyle: '#123456', lineWidth: 4, lineCap: 'round', lineJoin: 'round' });
    }
  });

  test('samples arriving one at a time settle the same curves the finished stroke has, each once', () => {
    const { ink, stroke } = setup();
    for (let count = 2; count <= points.length; count++) stroke.draw(at(points.slice(0, count)));
    expect(ink.named('quadraticCurveTo').map(call => call.slice(1))).toEqual(referenceCurves(points));
    // Each batch starts where the last one ended, so the pieces join into one path.
    expect(ink.named('moveTo').map(call => call.slice(1))).toEqual([[0, 0], [5, 0], [15, 5], [20, 20]]);
  });

  test('a batch of samples settles in one stroke call, and a call with nothing new adds no ink', () => {
    const { ink, stroke } = setup();
    stroke.draw(at(points));
    expect(ink.named('stroke')).toHaveLength(1);
    expect(ink.named('quadraticCurveTo').map(call => call.slice(1))).toEqual(referenceCurves(points));
    stroke.draw(at(points));
    expect(ink.named('stroke')).toHaveLength(1);
  });

  test('the tail runs from the last settled point to the pen and is erased before the next one', () => {
    const { tail, stroke } = setup();
    stroke.draw(at(points.slice(0, 2)));
    expect(tail.named('moveTo')[0]).toEqual(['moveTo', 0, 0]);
    expect(tail.named('lineTo')[0]).toEqual(['lineTo', 0, 0]);
    expect(tail.named('clearRect')).toHaveLength(0);
    stroke.draw(at(points.slice(0, 4)));
    expect(tail.named('moveTo')[1]).toEqual(['moveTo', 15, 5]);
    expect(tail.named('lineTo')[1]).toEqual(['lineTo', 20, 10]);
    // The dot at the origin, in canvas pixels, padded by half the stroke's width and two pixels.
    expect(tail.named('clearRect')).toEqual([['clearRect', 420 - 10, 240 - 10, 20, 20]]);
    stroke.draw(at(points));
    expect(tail.named('clearRect')[1]).toEqual(['clearRect', 420 + 60 - 10, 240 + 20 - 10, 20 + 20, 20 + 20]);
  });

  test('clear empties both canvases and a new stroke starts from its first point', () => {
    const { ink, tail, stroke } = setup();
    stroke.draw(at(points));
    stroke.clear();
    for (const layer of [ink, tail]) expect(layer.calls.slice(-2)).toEqual([['setTransform', 1, 0, 0, 1, 0, 0], ['clearRect', 0, 0, 400, 300]]);
    const before = ink.calls.length;
    stroke.draw(at(points));
    expect(ink.calls).toHaveLength(before);
    stroke.begin(element, viewport, 2);
    stroke.draw(at(points.slice(0, 3)));
    expect(ink.named('moveTo').at(-1)).toEqual(['moveTo', 0, 0]);
  });

  test('replaying at a new pixel ratio places the entire stroke on the resized canvas', () => {
    const { ink, stroke } = setup();
    stroke.draw(at(points));
    stroke.begin(element, viewport, 1);
    stroke.draw(at(points));
    expect(ink.named('setTransform').at(-1)).toEqual(['setTransform', 2, 0, 0, 2, 210, 120]);
    expect(ink.named('quadraticCurveTo')).toHaveLength(referenceCurves(points).length * 2);
    expect(ink.named('moveTo').at(-1)).toEqual(['moveTo', 0, 0]);
  });

  test('a pen that cannot be built piece by piece is erased and painted whole on the tail canvas each sample', () => {
    const ink = fakeContext(), tail = fakeContext(), painted: number[] = [];
    const stroke = new LiveStroke(ink.context, tail.context);
    stroke.begin({ ...element, pen: 'fountain' }, viewport, 2, (_context, drawn) => { painted.push(drawn.points!.length); });
    // The scene's own drawing places the element, so the canvases carry the camera only.
    expect(tail.calls).toEqual([['setTransform', 4, 0, 0, 4, 20, 40]]);
    stroke.draw(at(points.slice(0, 3)));
    stroke.draw(at(points));
    expect(painted).toEqual([3, 6]);
    expect(ink.named('stroke')).toHaveLength(0);
    // The second sample first erases the first one's box: the stroke's bounds padded by twice its width and four pixels.
    expect(tail.named('clearRect')).toEqual([['clearRect', 20 + 400 - 36, 40 + 200 - 36, 40 + 72, 72]]);
    stroke.clear();
    expect(tail.calls.slice(-2)).toEqual([['setTransform', 1, 0, 0, 1, 0, 0], ['clearRect', 0, 0, 400, 300]]);
  });
});
