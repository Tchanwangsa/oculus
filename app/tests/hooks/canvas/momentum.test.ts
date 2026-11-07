import { afterEach, beforeEach, describe, expect, spyOn, test } from 'bun:test';
import { createCanvasDocument } from '@/lib/canvas/model';
import { A4, type Paper } from '@/lib/canvas/paper';
import { createHarness, pointer, type Harness } from './harness';

// A fake clock and frame queue, so a glide runs frame by frame at 16 ms a frame.
let clock = 0, frames = new Map<number, FrameRequestCallback>(), nextFrame = 1;
let h: Harness;
const view = () => h.editor.documentRef.current.viewport;
const last = () => h.published[h.published.length - 1];
function harness(options: Parameters<typeof createHarness>[0] = {}, document = createCanvasDocument()) {
  h = createHarness(options, document);
  globalThis.requestAnimationFrame = (callback => { frames.set(nextFrame, callback); return nextFrame++; }) as typeof requestAnimationFrame;
  globalThis.cancelAnimationFrame = id => { frames.delete(id); };
  h.editor.setTool('hand'); h.render();
}
/** Runs up to `count` frames; returns how many ran before the queue emptied. */
function run(count = 1000) {
  let ran = 0;
  while (frames.size && ran < count) {
    const batch = [...frames.values()];
    frames.clear(); clock += 16; ran++;
    for (const callback of batch) callback(clock);
  }
  return ran;
}
/** A finger flicked `step` pixels every 16 ms for three samples from `from`, then lifted 2 ms after the last. */
function flick(type: string, from: [number, number], step: [number, number], id = 1) {
  h.editor.pointerDown(pointer(id, type, ...from));
  for (let index = 1; index <= 3; index++) {
    clock += 16;
    h.editor.pointerMove(pointer(id, type, from[0] + step[0] * index, from[1] + step[1] * index, { buttons: 1 }));
  }
  clock += 2;
  h.editor.pointerUp(pointer(id, type, from[0] + step[0] * 3, from[1] + step[1] * 3));
}
let now: ReturnType<typeof spyOn>;
beforeEach(() => { clock = 0; frames = new Map(); now = spyOn(performance, 'now').mockImplementation(() => clock); });
afterEach(() => now.mockRestore());

describe('a released pan glides', () => {
  test('a touch flick keeps moving, decelerates and saves the camera once where it stops', () => {
    harness();
    flick('touch', [100, 100], [0, 32]);
    expect(view()).toEqual({ x: 0, y: 96, zoom: 1 });
    expect(h.published).toHaveLength(0);
    run(1);
    const first = view().y - 96;
    run(1);
    expect(view().y - 96 - first).toBeLessThan(first);
    expect(h.published).toHaveLength(0);
    run();
    // 2 px/ms at 0.998 a millisecond travels just under 2 / -ln(0.998) ≈ 999 px.
    expect(view().y - 96).toBeGreaterThan(950);
    expect(view().y - 96).toBeLessThan(1000);
    expect(view().x).toBe(0);
    expect(h.published).toHaveLength(1);
    expect(last().viewport).toEqual(view());
  });
  test('a pen pan glides too; a mouse drag stops where it is released', () => {
    harness();
    flick('pen', [100, 100], [32, 0]);
    run();
    expect(view().x).toBeGreaterThan(900);
    harness();
    flick('mouse', [100, 100], [32, 0]);
    expect(h.published).toHaveLength(1);
    run();
    expect(view()).toEqual({ x: 96, y: 0, zoom: 1 });
  });
  test('a finger lifted after holding still does not glide', () => {
    harness();
    h.editor.pointerDown(pointer(1, 'touch', 100, 100));
    clock += 16; h.editor.pointerMove(pointer(1, 'touch', 100, 160, { buttons: 1 }));
    clock += 80; h.editor.pointerUp(pointer(1, 'touch', 100, 160));
    expect(h.published).toHaveLength(1);
    run();
    expect(view().y).toBe(60);
  });
  test('two fingers lifted while moving glide; a pinch does not', () => {
    harness();
    h.editor.pointerDown(pointer(1, 'touch', 100, 100)); h.editor.pointerDown(pointer(2, 'touch', 200, 100));
    for (let index = 1; index <= 3; index++) {
      clock += 16;
      h.editor.pointerMove(pointer(1, 'touch', 100, 100 + index * 32)); h.editor.pointerMove(pointer(2, 'touch', 200, 100 + index * 32));
    }
    clock += 2; h.editor.pointerUp(pointer(1, 'touch', 100, 196)); h.editor.pointerUp(pointer(2, 'touch', 200, 196));
    expect(h.published).toHaveLength(0);
    run();
    expect(view().y).toBeGreaterThan(1000);
    expect(h.published).toHaveLength(1);
    harness();
    h.editor.pointerDown(pointer(1, 'touch', 100, 100)); h.editor.pointerDown(pointer(2, 'touch', 200, 100));
    for (let index = 1; index <= 3; index++) { clock += 16; h.editor.pointerMove(pointer(2, 'touch', 200 + index * 40, 100)); }
    clock += 2; h.editor.pointerUp(pointer(1, 'touch', 100, 100)); h.editor.pointerUp(pointer(2, 'touch', 320, 100));
    expect(h.published).toHaveLength(1);
    const pinched = view();
    run();
    expect(view()).toEqual(pinched);
  });
});

describe('a glide stops', () => {
  test('at a touch, which then holds the page instead of drawing', () => {
    harness();
    flick('touch', [100, 100], [0, 32]);
    run(5);
    h.editor.setTool('freedraw'); h.render();
    const stopped = view().y;
    h.editor.pointerDown(pointer(2, 'touch', 300, 300));
    expect(h.published).toHaveLength(1);
    run();
    expect(view().y).toBe(stopped);
    h.editor.pointerMove(pointer(2, 'touch', 300, 340, { buttons: 1 }));
    h.editor.pointerUp(pointer(2, 'touch', 300, 340)); h.render();
    expect(view().y).toBe(stopped + 40);
    expect(h.editor.documentRef.current.elements).toEqual([]);
  });
  test('at a pen contact, which then draws', () => {
    harness();
    flick('touch', [100, 100], [0, 32]);
    run(5);
    h.editor.setTool('freedraw'); h.render();
    const stopped = view().y;
    h.editor.pointerDown(pointer(2, 'pen', 300, 300));
    h.editor.pointerMove(pointer(2, 'pen', 330, 340, { buttons: 1 }));
    h.editor.pointerUp(pointer(2, 'pen', 330, 340)); h.render();
    run();
    expect(view().y).toBe(stopped);
    expect(h.editor.documentRef.current.elements).toHaveLength(1);
  });
  test('at the wheel or a camera button', () => {
    harness({ size: { width: 800, height: 600 } });
    flick('touch', [100, 100], [0, 32]);
    run(5);
    const stopped = view().y;
    h.wheel({ deltaY: 10 });
    run();
    expect(view().y).toBe(stopped - 10);
    flick('touch', [100, 100], [0, 32]);
    run(5);
    h.editor.zoom(2);
    const zoomed = view();
    run();
    expect(view()).toEqual(zoomed);
  });
  test('at the end of a notebook’s pages, and saves the camera there once', () => {
    const paper: Paper = { pages: [{ id: 'p1', ...A4 }, { id: 'p2', ...A4 }] };
    harness({ kind: 'notebook', size: { width: 1000, height: 800 } }, { ...createCanvasDocument(), paper });
    const { zoom, x } = view();
    flick('touch', [500, 600], [0, -200]);
    const ran = run();
    expect(ran).toBeLessThan(60);
    expect(view().y + 1708 * zoom).toBeCloseTo(0.6 * 800);
    expect(view().x).toBe(x);
    expect(h.published).toHaveLength(1);
    expect(last().viewport).toEqual(view());
  });
});
