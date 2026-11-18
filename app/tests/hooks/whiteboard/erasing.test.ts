import { describe, expect, test } from 'bun:test';
import { createWhiteboardDocument, type WhiteboardElement } from '@/lib/whiteboard/model';
import { createHarness, key, pointer, type Harness } from './harness';

// At the harness's camera a client pixel is a world unit, so a small partial eraser (6 px) reaches 7 units of a 2-wide stroke.
const ink: WhiteboardElement = { id: 'ink', type: 'freedraw', x: 10, y: 20, width: 100, height: 0, stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 1, points: [{ x: 0, y: 0 }, { x: 100, y: 0 }] };
const under: WhiteboardElement = { ...ink, id: 'under' };
const box: WhiteboardElement = { ...ink, id: 'box', type: 'rectangle', x: 300, y: 300, width: 40, height: 40, points: undefined };

function setup(elements: WhiteboardElement[], options: Parameters<typeof createHarness>[0] = {}) {
  const h = createHarness(options);
  h.render({ ...createWhiteboardDocument(), elements });
  h.editor.setTool('eraser'); h.render();
  return h;
}
const ids = (h: Harness) => h.editor.documentRef.current.elements.map(element => element.id);

describe('a whole-stroke erase is a ghost until release', () => {
  test('nothing leaves the scene mid-gesture, and the release commits once', () => {
    const h = setup([ink, box]);
    h.editor.pointerDown(pointer(1, 'mouse', 40, 20));
    h.editor.pointerMove(pointer(1, 'mouse', 320, 300));
    expect(ids(h)).toEqual(['ink', 'box']);
    expect(h.published).toHaveLength(0);
    h.editor.pointerUp(pointer(1, 'mouse', 320, 300)); h.render();
    expect(ids(h)).toEqual([]);
    expect(h.published).toHaveLength(1);
    h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements).toEqual([ink, box]);
    expect(h.editor.canUndo).toBe(false);
  });
  test('what the gesture already took is passed over, so the eraser reaches what lies beneath', () => {
    const h = setup([under, ink]);
    h.editor.pointerDown(pointer(1, 'mouse', 40, 20));
    h.editor.pointerMove(pointer(1, 'mouse', 40, 20));
    h.editor.pointerUp(pointer(1, 'mouse', 40, 20)); h.render();
    expect(ids(h)).toEqual([]);
  });
  test('a cancel or Escape restores everything', () => {
    const h = setup([ink, box]);
    h.editor.pointerDown(pointer(1, 'pen', 40, 20));
    h.editor.pointerMove(pointer(1, 'pen', 320, 300));
    h.editor.pointerCancel(pointer(1, 'pen', 320, 300)); h.render();
    h.editor.pointerDown(pointer(2, 'mouse', 40, 20));
    h.editor.keyDown(key('Escape'));
    h.editor.pointerUp(pointer(2, 'mouse', 40, 20)); h.render();
    expect(h.editor.documentRef.current.elements).toEqual([ink, box]);
    expect(h.published).toHaveLength(0);
  });
});

describe('partial erasing', () => {
  const options = { eraserMode: 'partial' as const, eraserSize: 'small' as const };
  test('a sweep cuts the stroke into pieces that commit above it as one undo step', () => {
    const h = setup([ink, box], options);
    // Both samples are well clear of the stroke; only the swept segment between them crosses it.
    h.editor.pointerDown(pointer(1, 'mouse', 60, -20));
    h.editor.pointerMove(pointer(1, 'mouse', 60, 60));
    expect(h.editor.documentRef.current.elements).toEqual([ink, box]);
    expect(h.published).toHaveLength(0);
    h.editor.pointerUp(pointer(1, 'mouse', 60, 60)); h.render();
    expect(h.published).toHaveLength(1);
    const [left, right, top] = h.editor.documentRef.current.elements;
    expect(left).toMatchObject({ id: 'ink', type: 'freedraw', x: 10, y: 20 });
    expect(right.id).not.toBe('ink');
    expect(top).toBe(box);
    expect(left.x + left.points!.at(-1)!.x).toBeLessThan(53);
    expect(right.x).toBeGreaterThan(67);
    expect(right.x + right.points!.at(-1)!.x).toBe(110);
    h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements).toEqual([ink, box]);
    h.editor.history(true); h.render();
    expect(ids(h)).toEqual(['ink', right.id, 'box']);
  });
  test('coalesced samples sweep too', () => {
    const h = setup([ink], options);
    h.editor.pointerDown(pointer(1, 'pen', 30, -40));
    h.editor.pointerMove(pointer(1, 'pen', 90, -40, {}, [{ clientX: 30, clientY: 60 }, { clientX: 90, clientY: 60 }]));
    h.editor.pointerUp(pointer(1, 'pen', 90, -40)); h.render();
    expect(ids(h)).toHaveLength(3);
  });
  test('shapes go whole; a stroke swept end to end is deleted', () => {
    const h = setup([ink, box], options);
    h.editor.pointerDown(pointer(1, 'mouse', 0, 20));
    h.editor.pointerMove(pointer(1, 'mouse', 120, 20));
    h.editor.pointerMove(pointer(1, 'mouse', 320, 300));
    h.editor.pointerUp(pointer(1, 'mouse', 320, 300)); h.render();
    expect(ids(h)).toEqual([]);
    expect(h.published).toHaveLength(1);
  });
  test('a cancel restores the stroke uncut', () => {
    const h = setup([ink], options);
    h.editor.pointerDown(pointer(1, 'pen', 60, -20));
    h.editor.pointerMove(pointer(1, 'pen', 60, 60));
    h.editor.pointerCancel(pointer(1, 'pen', 60, 60)); h.render();
    expect(h.editor.documentRef.current.elements).toEqual([ink]);
    expect(h.published).toHaveLength(0);
  });
});
