import { describe, expect, test } from 'bun:test';
import { createHarness, pointer, type Harness } from './harness';

function fakeLayer() {
  const calls: string[] = [];
  const record = (name: string) => () => { calls.push(name); };
  const context = {
    canvas: { width: 400, height: 300 }, setTransform: record('setTransform'), clearRect: record('clearRect'), beginPath: record('beginPath'),
    moveTo: record('moveTo'), lineTo: record('lineTo'), quadraticCurveTo: record('quadraticCurveTo'), stroke: record('stroke'),
  };
  return { calls, canvas: { getContext: () => context } as unknown as HTMLCanvasElement };
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

describe('freehand ink on the live layer', () => {
  test('each sample reaches the live layer once and the committed stroke is the one the scene alone would commit', () => {
    const plain = draw(createHarness());
    const h = createHarness(), ink = fakeLayer(), tail = fakeLayer();
    h.editor.inkCanvas.current = ink.canvas; h.editor.tailCanvas.current = tail.canvas;
    expect(draw(h)).toEqual(plain);
    expect(plain).toHaveLength(1);
    // Points: the doubled origin, two coalesced samples and three moves (the repeated one is dropped), so five bends.
    expect(ink.calls.filter(name => name === 'quadraticCurveTo')).toHaveLength(5);
    expect(tail.calls.filter(name => name === 'lineTo')).toHaveLength(5);
  });

  test('a cancelled stroke commits nothing', () => {
    const h = createHarness(), ink = fakeLayer(), tail = fakeLayer();
    h.editor.inkCanvas.current = ink.canvas; h.editor.tailCanvas.current = tail.canvas;
    h.editor.setTool('freedraw'); h.render();
    h.editor.pointerDown(pointer(1)); h.editor.pointerMove(pointer(1, 'pen', 30, 20));
    h.editor.pointerCancel(pointer(1));
    h.render();
    expect(h.published).toHaveLength(0);
    expect(h.editor.documentRef.current.elements).toHaveLength(0);
  });
});
