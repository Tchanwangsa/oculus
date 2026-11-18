import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { createWhiteboardDocument, type WhiteboardDocument, type WhiteboardPen } from '@/lib/whiteboard/model';
import { worldToScreen } from '@/lib/whiteboard/geometry';
import { PENS, isMarker } from '@/lib/whiteboard/pens';
import { drawElement } from '@/lib/whiteboard/render';
import { createHarness, key, pointer, type Harness } from './harness';

type Call = [string, ...unknown[]];
function fakeLayer() {
  const calls: Call[] = [];
  const canvas = { width: 400, height: 300, getContext: () => context };
  const context = new Proxy({ canvas }, {
    get: (target, name) => Reflect.has(target, name) ? Reflect.get(target, name) : (...args: unknown[]) => calls.push([String(name), ...args]),
    set: (target, name, value) => { calls.push([String(name), value]); return Reflect.set(target, name, value); },
  }) as unknown as CanvasRenderingContext2D;
  return { calls, context, canvas: canvas as unknown as HTMLCanvasElement, named: (name: string) => calls.filter(call => call[0] === name) };
}

const saved = { Path2D: globalThis.Path2D, getComputedStyle: globalThis.getComputedStyle, window: globalThis.window };
beforeEach(() => {
  Object.assign(globalThis, {
    Path2D: class {
      commands: Call[] = [];
      constructor(public data?: string) {}
      moveTo(...args: number[]) { this.commands.push(['moveTo', ...args]); }
      lineTo(...args: number[]) { this.commands.push(['lineTo', ...args]); }
      quadraticCurveTo(...args: number[]) { this.commands.push(['quadraticCurveTo', ...args]); }
    },
    getComputedStyle: () => ({ getPropertyValue: () => '#123456' }),
  });
});
afterEach(() => { Object.assign(globalThis, saved); });

function layers(h: Harness) {
  const ink = fakeLayer(), tail = fakeLayer(), scene = fakeLayer(), layer = { style: { opacity: '' } };
  h.editor.inkCanvas.current = ink.canvas; h.editor.tailCanvas.current = tail.canvas;
  h.editor.inkLayer.current = layer as HTMLDivElement;
  Object.assign(h.canvas, { getContext: () => scene.context });
  h.editor.setTool('freedraw'); h.render();
  return { ink, tail, scene, layer };
}
function draw(h: Harness) {
  h.editor.setTool('freedraw'); h.render();
  h.editor.pointerDown(pointer(1));
  h.editor.pointerMove(pointer(1, 'pen', 30, 20, {}, [{ clientX: 15, clientY: 12 }, { clientX: 22, clientY: 16 }]));
  h.editor.pointerMove(pointer(1, 'pen', 30, 20));
  h.editor.pointerMove(pointer(1, 'pen', 50, 25));
  h.editor.pointerUp(pointer(1, 'pen', 60, 40));
  h.render();
  return h.published.at(-1)!.elements.map(({ id: _id, seed: _seed, ...rest }) => rest);
}
function expectCleared(layer: ReturnType<typeof fakeLayer>) {
  expect(layer.calls.at(-1)).toEqual(['clearRect', 0, 0, layer.canvas.width, layer.canvas.height]);
}

describe('freehand ink on the live layer', () => {
  test('each sample reaches the live layer once and the fallback scene commits identical ink', () => {
    const plain = draw(createHarness());
    const h = createHarness(), { ink, tail } = layers(h);
    expect(draw(h)).toEqual(plain);
    expect(plain[0].points).toEqual([{ x: 0, y: 0 }, { x: 0, y: 0 }, { x: 5, y: 2 }, { x: 12, y: 6 }, { x: 20, y: 10 }, { x: 40, y: 15 }, { x: 50, y: 30 }]);
    expect(ink.named('quadraticCurveTo')).toHaveLength(5);
    expect(tail.named('lineTo')).toHaveLength(4);
  });

  test.each(['pointer cancel', 'tool switch', 'undo'])('%s clears both layers and discards only the unfinished stroke', action => {
    const h = createHarness(), { ink, tail } = layers(h);
    h.editor.pointerDown(pointer(1)); h.editor.pointerUp(pointer(1));
    h.editor.pointerDown(pointer(2));
    h.editor.pointerMove(pointer(2, 'pen', 30, 20));
    if (action === 'pointer cancel') h.editor.pointerCancel(pointer(2));
    if (action === 'tool switch') h.editor.setTool('highlighter');
    if (action === 'undo') h.editor.keyDown(key('z', { meta: true }));
    h.render();
    h.editor.pointerMove(pointer(2, 'pen', 35, 25, { buttons: action === 'pointer cancel' ? 0 : 1 }));
    h.editor.pointerUp(pointer(2, 'pen', 35, 25));
    expectCleared(ink); expectCleared(tail);
    expect(h.published).toHaveLength(1);
    expect(h.editor.documentRef.current.elements).toHaveLength(1);
  });

  test('only the live layer paints samples; the next scene frame takes over the committed stroke', () => {
    const h = createHarness({ size: { width: 400, height: 300 } }), { scene, ink, tail } = layers(h);
    h.editor.setTool('highlighter'); h.render();
    h.editor.pointerDown(pointer(1));
    h.frame();
    expect(scene.named('clearRect')).toHaveLength(1);
    expect(scene.named('stroke')).toHaveLength(0);
    h.editor.pointerMove(pointer(1, 'pen', 30, 40));
    expect(h.frame()).toBe(0);
    h.editor.redraw(); h.frame();
    expect(scene.named('stroke')).toHaveLength(0);
    h.editor.pointerUp(pointer(1, 'pen', 50, 60)); h.frame();
    expect(scene.named('stroke')).toHaveLength(1);
    expectCleared(ink); expectCleared(tail);
  });

  test.each<WhiteboardPen>(['ball', 'flatMarker'])('%s follows the same notebook page clip as the committed stroke', pen => {
    const document = { ...createWhiteboardDocument(), paper: { gap: 10, pages: [{ id: 'one', width: 100, height: 100 }, { id: 'two', width: 160, height: 100 }] } };
    const h = createHarness({ kind: 'notebook', size: { width: 208, height: 300 } }, document), { scene, ink, tail } = layers(h);
    const onPage = (x: number, y: number) => {
      const screen = worldToScreen({ x, y }, h.editor.documentRef.current.viewport);
      return pointer(1, 'pen', screen.x, screen.y);
    };
    if (isMarker(pen)) { h.editor.setTool('highlighter'); h.render(); h.editor.changeStyle({ pen }); h.render(); }
    h.editor.pointerDown(onPage(10, 90));
    h.editor.pointerMove(onPage(20, 110));
    // Ball's transform includes the stroke origin. The flat marker uses drawElement in world coordinates.
    const first = pen === 'ball' ? [-60, -90, 100, 100] : [-50, 0, 100, 100];
    const second = pen === 'ball' ? [-90, 20, 160, 100] : [-80, 110, 160, 100];
    for (const layer of [ink, tail]) expect(layer.named('rect').at(-1)).toEqual(['rect', ...first]);
    h.editor.pointerMove(onPage(40, 160));
    // The bounds still belong to page two even when the pen returns to page one.
    h.editor.pointerMove(onPage(10, 92));
    for (const layer of [ink, tail]) {
      expect(layer.named('clearRect')).toContainEqual(['clearRect', 0, 0, 400, 300]);
      expect(layer.named('rect').at(-1)).toEqual(['rect', ...second]);
      expect(layer.named('clip').length).toBeGreaterThan(0);
    }
    h.editor.pointerUp(onPage(10, 92)); h.frame();
    expect(scene.named('rect')).toContainEqual(['rect', -80, 110, 160, 100]);
    expectCleared(ink); expectCleared(tail);
  });

  test('a DPR-only change resizes every backing store at native page zoom 1.3 and replays the stroke', () => {
    let ratioChanged: (() => void) | undefined;
    const win = { devicePixelRatio: 2, getSelection: () => null, matchMedia: () => ({
      addEventListener: (_: string, listener: () => void) => { ratioChanged = listener; }, removeEventListener() {},
    }) };
    Object.assign(globalThis, { window: win });
    const size = { width: 311 * 1.3, height: 201 * 1.3 };
    const h = createHarness({ size }), { ink, tail } = layers(h);
    const left = 50 * 1.3, top = 45 * 1.3;
    Object.assign(h.canvas, { getBoundingClientRect: () => ({ left, top, ...size }) });
    h.resize(size);
    h.editor.pointerDown(pointer(1, 'pen', left + 13, top + 26));
    h.editor.pointerMove(pointer(1, 'pen', left + 26, top + 39));
    for (const canvas of [h.canvas, ink.canvas, tail.canvas]) expect(canvas).toMatchObject({ width: 809, height: 523 });
    win.devicePixelRatio = 1;
    ratioChanged!();
    for (const canvas of [h.canvas, ink.canvas, tail.canvas]) expect(canvas).toMatchObject({ width: 404, height: 261 });
    for (const layer of [ink, tail]) expect(layer.named('setTransform').at(-1)).toEqual(['setTransform', 1, 0, 0, 1, 13, 26]);
    expect(ink.named('quadraticCurveTo')).toHaveLength(2);
    expect(tail.named('lineTo').at(-1)).toEqual(['lineTo', 13, 13]);
  });

  test('a shared stroke arrives during a local annotation on a locked PDF page', () => {
    const doc = createWhiteboardDocument(), backdrop = { id: 'pdf', type: 'image' as const, locked: true, x: 0, y: 0, width: 500, height: 700, stroke: '#111', strokeWidth: 1, fill: 'transparent', style: 'clean' as const, seed: 1 };
    const initial: WhiteboardDocument = { ...doc, elements: [backdrop] };
    const h = createHarness({ localViewport: true, size: { width: 400, height: 300 } }, initial), { ink, tail, scene } = layers(h);
    h.editor.pointerDown(pointer(1)); h.editor.pointerMove(pointer(1, 'pen', 80, 40));
    h.editor.pointerDown(pointer(2, 'touch')); h.editor.pointerCancel(pointer(2, 'touch'));
    const remote = { ...backdrop, type: 'freedraw' as const, locked: false, id: 'remote', points: [{ x: 0, y: 0 }, { x: 10, y: 10 }] };
    h.render({ ...initial, elements: [backdrop, remote] }); h.frame();
    expect(scene.named('stroke')).toHaveLength(1);
    scene.calls.length = 0;
    h.editor.pointerUp(pointer(1, 'pen', 90, 80)); h.frame();
    expect(scene.named('stroke')).toHaveLength(2);
    expect(h.published).toHaveLength(1);
    expect(h.published[0].elements.slice(0, 2)).toEqual([backdrop, remote]);
    expect(h.published[0].elements[2].points?.at(-1)).toEqual({ x: 80, y: 70 });
    expectCleared(ink); expectCleared(tail);
  });

  test.each([...PENS.filter(pen => pen !== 'ball'), 'flatMarker'] as WhiteboardPen[])('%s uses the committed pen passes on the tail', pen => {
    const h = createHarness(), { tail, layer } = layers(h);
    if (isMarker(pen)) { h.editor.setTool('highlighter'); h.render(); }
    h.editor.changeStyle({ pen, opacity: 0.37, strokeWidth: 7 }); h.render();
    h.editor.pointerDown(pointer(1)); tail.calls.length = 0;
    h.editor.pointerMove(pointer(1, 'pen', 20, 40));
    h.editor.pointerUp(pointer(1, 'pen', 20, 40));
    const reference = fakeLayer();
    drawElement(reference.context, { ...h.published[0].elements[0] });
    expect(tail.calls.slice(tail.calls.findIndex(([name]) => name === 'save'))).toEqual(reference.calls);
    expect(layer.style.opacity).toBe('1');
  });

  test('the round highlighter fades its settled ink and tail together', () => {
    const h = createHarness(), { ink, tail, layer } = layers(h);
    h.editor.setTool('highlighter'); h.render();
    h.editor.pointerDown(pointer(1)); h.editor.pointerMove(pointer(1, 'pen', 20, 40));
    h.editor.pointerUp(pointer(1, 'pen', 20, 40));
    const mark = h.published[0].elements[0];
    expect(mark.pen).toBe('roundMarker');
    expect(layer.style.opacity).toBe(String(mark.opacity));
    for (const part of [ink, tail]) {
      expect(part.context.lineWidth).toBe(mark.strokeWidth);
      expect(part.context.strokeStyle).toBe(mark.stroke);
      expect(part.named('globalAlpha')).toHaveLength(0);
    }
  });

  test('a fresh Pencil contact before the paint frame keeps the recovered stroke and its new live tip', () => {
    const h = createHarness({ size: { width: 400, height: 300 } }), { ink, tail, scene } = layers(h);
    h.editor.pointerDown(pointer(1)); h.editor.pointerCancel(pointer(1));
    h.editor.pointerMove(pointer(1, 'pen', 10, 10, { buttons: 1 }));
    h.editor.pointerMove(pointer(1, 'pen', 20, 20));
    h.editor.pointerDown(pointer(2, 'pen', 90, 60));
    h.frame();
    expect(scene.named('stroke')).toHaveLength(1);
    expect(tail.calls.at(-1)).toEqual(['stroke']);
    h.editor.pointerUp(pointer(2, 'pen', 95, 70)); h.frame();
    expect(h.published.at(-1)!.elements.map(mark => [mark.x, mark.y, mark.points?.at(-1)])).toEqual([
      [10, 10, { x: 10, y: 10 }], [90, 60, { x: 5, y: 10 }],
    ]);
    expectCleared(ink); expectCleared(tail);
  });
});
