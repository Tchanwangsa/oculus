import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import { createHarness, pointer, type Harness } from './harness';

const shape: CanvasElement = { id: 'shape', type: 'rectangle', x: 100, y: 100, width: 40, height: 30, stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 1 };
const page: CanvasElement = { ...shape, id: 'page', type: 'image', x: 0, y: 0, width: 500, height: 700, naturalWidth: 1, naturalHeight: 1, dataUrl: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=' };
const ink: CanvasElement = { ...shape, id: 'ink', type: 'freedraw', x: 180, y: 100, width: 40, height: 30, points: [{ x: 0, y: 0 }, { x: 40, y: 30 }] };
function setup(elements: CanvasElement[] = [page, shape, ink]) {
  const h = createHarness();
  h.render({ ...createCanvasDocument(), elements });
  return h;
}
function drag(h: Harness, from: [number, number], to: [number, number], shiftKey = false) {
  h.editor.pointerDown(pointer(1, 'mouse', ...from, { shiftKey }));
  h.editor.pointerMove(pointer(1, 'mouse', ...to, { shiftKey }));
  h.editor.pointerUp(pointer(1, 'mouse', ...to, { shiftKey }));
  h.render();
}

describe('box selection encloses whole objects', () => {
  test('crossing a page or part of a shape selects neither', () => {
    const h = setup();
    drag(h, [-10, -10], [120, 120]);
    expect(h.editor.selection).toEqual([]);
    expect(h.published).toHaveLength(0);
  });
  test('a box starting on an unselected page selects only the enclosed annotations', () => {
    const h = setup();
    drag(h, [80, 80], [230, 140]);
    expect(h.editor.selection).toEqual(['shape', 'ink']);
    expect(h.editor.documentRef.current.elements).toEqual([page, shape, ink]);
    expect(h.published).toHaveLength(0);
  });
  test('reverse-direction selection uses the same enclosure rule', () => {
    const h = setup();
    drag(h, [230, 140], [80, 80]);
    expect(h.editor.selection).toEqual(['shape', 'ink']);
  });
  test('a page is included only when its entire bounds are enclosed', () => {
    const h = setup();
    drag(h, [-10, -10], [510, 710]);
    expect(h.editor.selection).toEqual(['page', 'shape', 'ink']);
  });
  test('click selects a page and a subsequent drag moves it with undo', () => {
    const h = setup();
    drag(h, [300, 300], [300, 300]);
    expect(h.editor.selection).toEqual(['page']);
    drag(h, [300, 300], [330, 320]);
    expect(h.editor.documentRef.current.elements[0]).toMatchObject({ id: 'page', x: 30, y: 20 });
    h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements).toEqual([page, shape, ink]);
  });
  test('Shift adds fully enclosed strokes while preserving the existing selection', () => {
    const h = setup();
    drag(h, [80, 80], [150, 140]);
    expect(h.editor.selection).toEqual(['shape']);
    drag(h, [160, 80], [230, 140], true);
    expect(h.editor.selection).toEqual(['shape', 'ink']);
  });
  test('a stroke must fit entirely, including a backwards path and a dot', () => {
    const backwards = { ...ink, x: 200, y: 200, points: [{ x: 0, y: 0 }, { x: -50, y: -50 }] };
    const dot = { ...ink, id: 'dot', x: 180, y: 180, points: [{ x: 0, y: 0 }] };
    const h = setup([page, backwards, dot]);
    drag(h, [160, 180], [210, 210]);
    expect(h.editor.selection).toEqual(['dot']);
  });
});
