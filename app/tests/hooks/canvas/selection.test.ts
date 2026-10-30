import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import { createHarness, key, pointer, type Harness } from './harness';

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

test('a canvas press clears the chat selection before giving the drawing focus', () => {
  const previousWindow = globalThis.window;
  const calls: string[] = [];
  Object.assign(globalThis, { window: { getSelection: () => ({ removeAllRanges: () => calls.push('clear caret') }) } });
  try {
    const h = setup();
    h.editor.root.current = { focus: () => calls.push('focus canvas') } as unknown as HTMLDivElement;
    h.editor.pointerDown(pointer(1, 'mouse', 300, 300));
    expect(calls).toEqual(['clear caret', 'focus canvas']);
    h.editor.pointerUp(pointer(1, 'mouse', 300, 300));
  } finally { Object.assign(globalThis, { window: previousWindow }); }
});


describe('moving from inside the selection frame', () => {
  const a: CanvasElement = { ...ink, id: 'a', x: 100, y: 100, width: 100, height: 100, points: [{ x: 0, y: 0 }, { x: 100, y: 100 }] };
  const b: CanvasElement = { ...a, id: 'b', x: 300 };
  function selectedGroup(backdrop = true) {
    const h = setup(backdrop ? [page, a, b] : [a, b]);
    drag(h, [80, 80], [420, 220]);
    expect(h.editor.selection).toEqual(['a', 'b']);
    return h;
  }
  test('the gap between strokes moves the whole group, with or without a page underneath', () => {
    for (const backdrop of [true, false]) {
      const h = selectedGroup(backdrop);
      drag(h, [250, 150], [280, 170]);
      expect(h.editor.selection).toEqual(['a', 'b']);
      expect(h.editor.documentRef.current.elements.filter(el => el.type === 'freedraw')).toEqual([
        { ...a, x: 130, y: 120 }, { ...b, x: 330, y: 120 },
      ]);
      if (backdrop) expect(h.editor.documentRef.current.elements[0]).toEqual(page);
      h.editor.history(); h.render();
      expect(h.editor.documentRef.current.elements).toEqual(backdrop ? [page, a, b] : [a, b]);
    }
  });
  test('a Pencil can drag the group from the gap, and an interior tap keeps it selected', () => {
    const h = selectedGroup();
    h.editor.pointerDown(pointer(1, 'pen', 250, 150)); h.editor.pointerUp(pointer(1, 'pen', 250, 150)); h.render();
    expect(h.editor.selection).toEqual(['a', 'b']);
    expect(h.published).toHaveLength(0);
    h.editor.pointerDown(pointer(1, 'pen', 250, 150));
    h.editor.pointerMove(pointer(1, 'pen', 280, 170)); h.editor.pointerUp(pointer(1, 'pen', 280, 170)); h.render();
    expect(h.editor.selection).toEqual(['a', 'b']);
    expect(h.editor.documentRef.current.elements[1]).toMatchObject({ id: 'a', x: 130, y: 120 });
  });
  test('a single hollow stroke can be moved from its empty middle', () => {
    const hollow: CanvasElement = { ...a, width: 200, points: [{ x: 0, y: 0 }, { x: 200, y: 0 }, { x: 200, y: 100 }, { x: 0, y: 100 }] };
    const h = setup([hollow]);
    drag(h, [80, 80], [320, 220]);
    drag(h, [200, 150], [230, 170]);
    expect(h.editor.selection).toEqual(['a']);
    expect(h.editor.documentRef.current.elements).toEqual([{ ...hollow, x: 130, y: 120 }]);
  });
  test('hover inside the frame shows the move cursor; outside keeps ordinary selection', () => {
    const h = selectedGroup();
    h.editor.pointerMove(pointer(1, 'mouse', 250, 150)); h.render();
    expect(h.editor.cursor).toBe('move');
    h.editor.pointerMove(pointer(1, 'mouse', 450, 250)); h.render();
    expect(h.editor.cursor).toBe('default');
    drag(h, [550, 750], [550, 750]);
    expect(h.editor.selection).toEqual([]);
  });
  test('the frame padding stays draggable at different zoom levels', () => {
    for (const zoom of [1, 8]) {
      const h = selectedGroup();
      h.editor.zoom(zoom); h.render();
      drag(h, [100 * zoom - 2.5, 125 * zoom], [130 * zoom - 2.5, 145 * zoom]);
      expect(h.editor.selection).toEqual(['a', 'b']);
      expect(h.editor.documentRef.current.elements[1]).toMatchObject({ id: 'a', x: 130, y: 120 });
    }
  });
  test('resize handles take priority over moving the selection body', () => {
    const h = selectedGroup();
    drag(h, [400, 200], [460, 220]);
    const moved = h.editor.documentRef.current.elements.find(el => el.id === 'a')!;
    expect(moved.x).toBe(a.x); expect(moved.y).toBe(a.y);
    expect(moved.width).toBeGreaterThan(a.width);
  });
  test('Shift still toggles a selected stroke and adds another stroke inside the frame', () => {
    const h = selectedGroup();
    drag(h, [130, 130], [130, 130], true);
    expect(h.editor.selection).toEqual(['b']);
    drag(h, [130, 130], [130, 130], true);
    expect(h.editor.selection).toEqual(['b', 'a']);
    expect(h.published).toHaveLength(0);
  });
  test('a single line keeps point editing rather than an invisible rectangular drag area', () => {
    const line: CanvasElement = { ...a, type: 'line', width: 200, height: 200, points: [{ x: 0, y: 0 }, { x: 200, y: 200 }] };
    const h = setup([line]);
    h.editor.keyDown(key('a', { meta: true })); h.render();
    expect(h.editor.selection).toEqual(['a']);
    drag(h, [250, 130], [250, 130]);
    expect(h.editor.selection).toEqual([]);
    expect(h.published).toHaveLength(0);
  });
});
