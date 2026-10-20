import { beforeEach, describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import { createHarness, key, pointer, type Editor, type Harness } from './harness';

// Pencil and touch behaviour confirmed on an iPad with Apple Pencil; these keep it from regressing.
let h: Harness;
const editor = () => h.editor;
const last = () => h.published[h.published.length - 1];
function select(tool: Parameters<Editor['setTool']>[0]) { editor().setTool(tool); h.render(); }
function stroke(id: number, type = 'pen') {
  editor().pointerDown(pointer(id, type));
  editor().pointerMove(pointer(id, type, 30, 20));
  editor().pointerUp(pointer(id, type, 40, 30));
}
function setup(eraserAutoReturn = false) { h = createHarness({ localViewport: true, eraserAutoReturn }); select('freedraw'); }
beforeEach(() => setup());

describe('erasing beside imported pages', () => {
  const image: CanvasElement = {
    id: 'page', type: 'image', x: 0, y: 0, width: 200, height: 200,
    stroke: '#000', fill: 'transparent', strokeWidth: 1, style: 'clean', seed: 1,
    dataUrl: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=', naturalWidth: 200, naturalHeight: 200,
  };
  test('eraser taps and drags leave an image untouched without adding undo steps', () => {
    h.render({ ...createCanvasDocument(), elements: [image] });
    select('eraser');
    stroke(1, 'mouse'); h.render();
    editor().pointerDown(pointer(2, 'mouse', 100, 100)); editor().pointerUp(pointer(2, 'mouse', 100, 100)); h.render();
    expect(editor().documentRef.current.elements).toEqual([image]);
    expect(h.published).toHaveLength(0);
    expect(editor().canUndo).toBe(false);
  });
  test('eraser deletes ink over or under an image and undo restores only the ink', () => {
    const ink: CanvasElement = { ...image, id: 'ink', type: 'freedraw', x: 10, y: 10, width: 30, height: 20, points: [{ x: 0, y: 0 }, { x: 30, y: 20 }] };
    for (const elements of [[image, ink], [ink, image]]) {
      h = createHarness();
      h.render({ ...createCanvasDocument(), elements });
      select('eraser'); stroke(1, 'mouse'); h.render();
      expect(editor().documentRef.current.elements).toEqual([image]);
      editor().history(); h.render();
      expect(editor().documentRef.current.elements).toEqual(elements);
    }
  });
  test('selecting an image then using the trash handler deletes it and remains undoable', () => {
    h.render({ ...createCanvasDocument(), elements: [image] });
    select('select');
    editor().pointerDown(pointer(1, 'mouse', 100, 100)); editor().pointerUp(pointer(1, 'mouse', 100, 100)); h.render();
    expect(editor().selection).toEqual(['page']);
    editor().deleteSelected(); h.render();
    expect(editor().documentRef.current.elements).toEqual([]);
    editor().history(); h.render();
    expect(editor().documentRef.current.elements).toEqual([image]);
  });
});

describe('Pencil and touch pointer lifecycle', () => {
  test('a pen move with the contact button recovers a missing down, while hover never inks', () => {
    editor().pointerMove(pointer(1, 'pen', 10, 10, { button: -1, buttons: 0, pressure: 0.5 }));
    editor().pointerUp(pointer(1));
    expect(h.published).toHaveLength(0);
    editor().pointerMove(pointer(1, 'pen', 20, 20, { button: -1, buttons: 1, pressure: 0.2 }));
    editor().pointerMove(pointer(1, 'pen', 30, 30, { button: -1, buttons: 1 }));
    editor().pointerUp(pointer(1, 'pen', 40, 40));
    expect(last()?.elements).toHaveLength(1);
    expect(last()?.elements[0].x).toBe(20);
  });
  test('a fresh pen down keeps the ink of a stroke whose up was lost, as its own stroke', () => {
    editor().pointerDown(pointer(1)); editor().pointerMove(pointer(1, 'pen', 30, 30));
    editor().pointerDown(pointer(1, 'pen', 100, 100)); editor().pointerUp(pointer(1, 'pen', 120, 120));
    expect(h.published).toHaveLength(2);
    expect(last()?.elements.map(element => [element.x, element.y, element.width, element.height])).toEqual([[10, 10, 20, 20], [100, 100, 20, 20]]);
  });
  test('rapid strokes can reuse one Pencil pointer id indefinitely', () => {
    for (let index = 0; index < 40; index++) stroke(1);
    expect(last()?.elements).toHaveLength(40);
  });
  test('capture and release failures neither stop drawing nor strand the next stroke', () => {
    const { setPointerCapture, releasePointerCapture } = h.canvas;
    try {
      h.canvas.setPointerCapture = () => { throw new Error('Pointer is no longer active'); };
      stroke(1);
      h.canvas.setPointerCapture = setPointerCapture;
      h.canvas.releasePointerCapture = () => { throw new Error('Pointer is no longer active'); };
      stroke(1); stroke(1);
      expect(last()?.elements).toHaveLength(3);
    } finally { h.canvas.setPointerCapture = setPointerCapture; h.canvas.releasePointerCapture = releasePointerCapture; }
  });
  test('a second pointer cannot start while a stroke runs, and a cancel drops the stroke', () => {
    editor().pointerDown(pointer(1)); editor().pointerDown(pointer(2, 'mouse', 50, 50));
    editor().pointerMove(pointer(2, 'mouse', 80, 80)); editor().pointerUp(pointer(2, 'mouse', 80, 80));
    expect(h.published).toHaveLength(0);
    editor().pointerMove(pointer(1, 'pen', 30, 30)); editor().pointerCancel(pointer(1));
    editor().pointerUp(pointer(1, 'pen', 40, 40));
    expect(h.published).toHaveLength(0);
    stroke(1);
    expect(last()?.elements).toHaveLength(1);
  });
  test('fingers draw until a Pencil is used; then one finger pans without ink', () => {
    stroke(1, 'touch'); expect(last()?.elements).toHaveLength(1);
    stroke(2); expect(last()?.elements).toHaveLength(2);
    stroke(3, 'touch'); expect(last()?.elements).toHaveLength(2);
    expect(editor().documentRef.current.viewport).toEqual({ x: 30, y: 20, zoom: 1 });
    expect(h.views[h.views.length - 1]).toEqual({ x: 30, y: 20, zoom: 1 });
    expect(editor().tool).toBe('freedraw');
  });
  test('a palm landing before the Pencil cannot block the pen stroke', () => {
    editor().pointerDown(pointer(1, 'touch'));
    editor().pointerDown(pointer(2)); editor().pointerMove(pointer(2, 'pen', 30, 30));
    editor().pointerUp(pointer(2, 'pen', 40, 40)); editor().pointerUp(pointer(1, 'touch', 300, 300));
    expect(last()?.elements).toHaveLength(1);
    expect(last()?.elements[0].width).toBe(30);
  });
  test('palm moves and cancels during a pen contact neither cancel nor navigate the stroke', () => {
    editor().pointerDown(pointer(1)); editor().pointerDown(pointer(2, 'touch'));
    editor().pointerMove(pointer(2, 'touch', 300, 300)); editor().pointerCancel(pointer(2, 'touch'));
    editor().pointerMove(pointer(1, 'pen', 30, 30)); editor().pointerUp(pointer(1, 'pen', 40, 40));
    expect(last()?.elements).toHaveLength(1);
    expect(editor().documentRef.current.viewport).toEqual({ x: 0, y: 0, zoom: 1 });
  });
  test('two fingers pinch with a drawing tool after pen use and add no ink', () => {
    stroke(1); editor().pointerDown(pointer(2, 'touch', 100, 100)); editor().pointerDown(pointer(3, 'touch', 200, 100));
    editor().pointerMove(pointer(3, 'touch', 300, 100)); editor().pointerUp(pointer(3, 'touch', 300, 100));
    editor().pointerMove(pointer(2, 'touch', 500, 100)); editor().pointerUp(pointer(2, 'touch', 500, 100));
    expect(editor().documentRef.current.viewport.zoom).toBe(2);
    expect(last()?.elements).toHaveLength(1);
  });
  test('two-finger navigation takes over from every tool without committing its preview', () => {
    for (const tool of ['select', 'hand', 'rectangle', 'ellipse', 'diamond', 'line', 'arrow', 'freedraw', 'text', 'eraser'] as const) {
      select(tool);
      const before = editor().documentRef.current.elements.length;
      editor().pointerDown(pointer(10, 'touch', 100, 100)); editor().pointerDown(pointer(11, 'touch', 200, 100));
      editor().pointerMove(pointer(11, 'touch', 210, 100)); editor().pointerUp(pointer(11, 'touch', 210, 100)); editor().pointerUp(pointer(10, 'touch', 100, 100));
      h.render();
      expect(editor().documentRef.current.elements).toHaveLength(before);
      expect(editor().tool).toBe(tool);
      expect(editor().textDraft).toBeNull();
    }
  });
  test('the release sample ends a quick down and up that had no move', () => {
    editor().pointerDown(pointer(1)); editor().pointerUp(pointer(1, 'pen', 30, 20));
    const ink = last()?.elements[0];
    expect(ink?.width).toBe(20); expect(ink?.height).toBe(10);
    expect(ink?.points?.[ink.points.length - 1]).toEqual({ x: 20, y: 10 });
  });
  test('dots and short marks are kept through a burst of distinct strokes', () => {
    editor().pointerDown(pointer(1)); editor().pointerUp(pointer(1));
    for (let id = 2; id <= 41; id++) { editor().pointerDown(pointer(id)); editor().pointerUp(pointer(id, 'pen', 11, 11)); }
    expect(last()?.elements).toHaveLength(41);
    expect(new Set(last()?.elements.map(element => element.id)).size).toBe(41);
  });
  test('coalesced samples stay in a curved stroke before simplification', () => {
    editor().pointerDown(pointer(1));
    editor().pointerMove(pointer(1, 'pen', 40, 10, {}, [{ clientX: 20, clientY: 40 }, { clientX: 30, clientY: 10 }]));
    editor().pointerUp(pointer(1, 'pen', 50, 40));
    const points = last()?.elements[0].points;
    expect(points).toContainEqual({ x: 10, y: 30 });
    expect(points?.[points.length - 1]).toEqual({ x: 40, y: 30 });
  });
  test('an acknowledgement with a fresh identity does not reset the next stroke', () => {
    stroke(1); const ack = structuredClone(last()!);
    editor().pointerDown(pointer(2)); h.render(ack);
    editor().pointerMove(pointer(2, 'pen', 30, 30)); editor().pointerUp(pointer(2, 'pen', 40, 40));
    expect(last()?.elements).toHaveLength(2);
  });
  test('a touch tap with the Text tool opens the overlay on release', () => {
    select('text');
    editor().pointerDown(pointer(1, 'touch', 50, 50)); h.render();
    expect(editor().textDraft).toBeNull();
    editor().pointerUp(pointer(1, 'touch', 50, 50)); h.render();
    expect(editor().textDraft?.element).toMatchObject({ type: 'text', x: 50, y: 50 });
  });
});

describe('Return after erasing', () => {
  test('is off by default: the eraser stays chosen after lifting', () => {
    stroke(1); select('eraser');
    editor().pointerDown(pointer(2, 'pen', 20, 15)); editor().pointerUp(pointer(2, 'pen', 20, 15)); h.render();
    expect(editor().tool).toBe('eraser');
    expect(last()?.elements).toHaveLength(0);
  });
  test('when on, a completed erase returns to the tool before the eraser', () => {
    setup(true); stroke(1); select('eraser');
    editor().pointerDown(pointer(2, 'pen', 20, 15)); expect(editor().tool).toBe('eraser');
    editor().pointerUp(pointer(2, 'pen', 20, 15)); h.render();
    expect(editor().tool).toBe('freedraw');
    expect(last()?.elements).toHaveLength(0);
  });
  test('the eraser chosen from the keyboard remembers the shape tool before it', () => {
    setup(true); select('rectangle');
    editor().keyDown(key('e')); h.render();
    expect(editor().tool).toBe('eraser');
    editor().pointerDown(pointer(1)); editor().pointerUp(pointer(1)); h.render();
    expect(editor().tool).toBe('rectangle');
  });
  test('a cancelled erase or a pinch that takes over does not switch tools', () => {
    setup(true); select('eraser');
    editor().pointerDown(pointer(1)); editor().pointerCancel(pointer(1)); h.render();
    expect(editor().tool).toBe('eraser');
    editor().pointerDown(pointer(2, 'touch')); editor().pointerDown(pointer(3, 'touch', 100, 100));
    editor().pointerUp(pointer(3, 'touch', 100, 100)); editor().pointerUp(pointer(2, 'touch')); h.render();
    expect(editor().tool).toBe('eraser');
    expect(editor().documentRef.current).toMatchObject({ elements: createCanvasDocument().elements });
  });
});
