import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import { createHarness, key, pointer, type Harness } from './harness';

const base = { stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const PNG = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=';
const page: CanvasElement = { id: 'page', type: 'image', x: 0, y: 0, width: 500, height: 700, naturalWidth: 1, naturalHeight: 1, dataUrl: PNG, locked: true, ...base };
const shape: CanvasElement = { id: 'shape', type: 'rectangle', x: 100, y: 100, width: 40, height: 30, ...base };
const ink: CanvasElement = { id: 'ink', type: 'freedraw', x: 180, y: 100, width: 40, height: 30, points: [{ x: 0, y: 0 }, { x: 40, y: 30 }], ...base };

function setup() {
  const h = createHarness();
  h.render({ ...createCanvasDocument(), elements: [page, shape, ink] });
  return h;
}
function drag(h: Harness, from: [number, number], to: [number, number]) {
  h.editor.pointerDown(pointer(1, 'mouse', ...from));
  h.editor.pointerMove(pointer(1, 'mouse', ...to));
  h.editor.pointerUp(pointer(1, 'mouse', ...to));
  h.render();
}

describe('a locked page is a backdrop', () => {
  test('clicking it selects nothing', () => {
    const h = setup();
    drag(h, [300, 300], [300, 300]);
    expect(h.editor.selection).toEqual([]);
  });
  test('dragging on it never moves it', () => {
    const h = setup();
    drag(h, [300, 300], [340, 320]);
    drag(h, [300, 300], [340, 320]);
    expect(h.editor.documentRef.current.elements[0]).toBe(page);
    expect(h.published).toHaveLength(0);
  });
  test('a box that encloses the whole page still leaves it out', () => {
    const h = setup();
    drag(h, [-10, -10], [510, 710]);
    expect(h.editor.selection).toEqual(['shape', 'ink']);
  });
  test('select all leaves it out', () => {
    const h = setup();
    h.editor.keyDown(key('a', { meta: true }));
    h.render();
    expect(h.editor.selection).toEqual(['shape', 'ink']);
  });
  test('the eraser removes the ink over it and never the page', () => {
    const h = setup();
    h.editor.setTool('eraser'); h.render();
    drag(h, [300, 300], [300, 300]);
    drag(h, [200, 115], [200, 115]);
    expect(h.editor.documentRef.current.elements.map(e => e.id)).toEqual(['page', 'shape']);
  });
  test('deleting a selection leaves it in place', () => {
    const h = setup();
    h.editor.keyDown(key('a', { meta: true })); h.render();
    h.editor.keyDown(key('Delete')); h.render();
    expect(h.editor.documentRef.current.elements.map(e => e.id)).toEqual(['page']);
  });
});
