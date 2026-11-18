import { describe, expect, test } from 'bun:test';
import { createWhiteboardDocument, type WhiteboardElement } from '@/lib/whiteboard/model';
import type { WhiteboardKind, Paper } from '@/lib/whiteboard/paper';
import { elementsAtPage, shownBounds } from '@/lib/whiteboard/pageHits';
import { createHarness, pointer, type Harness } from './harness';

// Two pages 100 x 200 with a gap of 10: page one spans y 0..200, page two y 210..410, both x -50..50. The camera is the
// default, so screen and world coordinates are the same.
const paper: Paper = { gap: 10, pages: [{ id: 'p1', width: 100, height: 200 }, { id: 'p2', width: 100, height: 200 }] };
const base = { stroke: '#000000', fill: '#000000', strokeWidth: 2, style: 'clean' as const, seed: 1 };
// Owned by page one (its centre is at y 200) and hanging 50 points onto page two, where the notebook hides it.
const hanging: WhiteboardElement = { id: 'hanging', type: 'rectangle', x: -10, y: 150, width: 20, height: 100, ...base };

const setup = (kind: WhiteboardKind) => createHarness({ kind }, { ...createWhiteboardDocument(), paper, elements: [hanging] });
function click(h: Harness, x: number, y: number) {
  h.editor.pointerDown(pointer(1, 'mouse', x, y)); h.editor.pointerUp(pointer(1, 'mouse', x, y)); h.render();
}
function drag(h: Harness, from: [number, number], to: [number, number]) {
  h.editor.pointerDown(pointer(1, 'mouse', ...from));
  h.editor.pointerMove(pointer(1, 'mouse', ...to, { buttons: 1 }));
  h.editor.pointerUp(pointer(1, 'mouse', ...to)); h.render();
}

describe('a notebook hits only what it draws', () => {
  test('a click on the hidden part of an element does not pick it; one on its page does', () => {
    const h = setup('notebook');
    click(h, 0, 230);
    expect(h.editor.selection).toEqual([]);
    click(h, 0, 180);
    expect(h.editor.selection).toEqual(['hanging']);
  });
  test('a box around the part its page shows selects it', () => {
    const h = setup('notebook');
    drag(h, [-40, 140], [40, 205]);
    expect(h.editor.selection).toEqual(['hanging']);
  });
  test('the eraser passes over the hidden part', () => {
    const h = setup('notebook');
    h.editor.setTool('eraser'); h.render();
    drag(h, [0, 230], [2, 240]);
    expect(h.editor.documentRef.current.elements).toHaveLength(1);
    drag(h, [0, 180], [2, 182]);
    expect(h.editor.documentRef.current.elements).toHaveLength(0);
  });
  test('a whiteboard holding paper hits the whole element, as a whiteboard does', () => {
    const h = setup('whiteboard');
    click(h, 0, 230);
    expect(h.editor.selection).toEqual(['hanging']);
    const boxed = setup('whiteboard');
    drag(boxed, [-40, 140], [40, 205]);
    expect(boxed.editor.selection).toEqual([]);
  });
});

describe('page hit helpers', () => {
  const other: WhiteboardElement = { ...hanging, id: 'other', y: 300 };
  const elements = [hanging, other];
  test('a point reaches only the elements of the page under it, and nothing on the desk', () => {
    expect(elementsAtPage(paper, elements, { x: 0, y: 230 }).map(e => e.id)).toEqual(['other']);
    expect(elementsAtPage(paper, elements, { x: 0, y: 100 }).map(e => e.id)).toEqual(['hanging']);
    expect(elementsAtPage(paper, elements, { x: 0, y: 205 })).toEqual([]);
    expect(elementsAtPage(paper, elements, { x: 80, y: 100 })).toEqual([]);
  });
  test('the same document gives the same array, so the hit-test scene is reused', () => {
    expect(elementsAtPage(paper, elements, { x: 0, y: 230 })).toBe(elementsAtPage(paper, elements, { x: 10, y: 300 }));
  });
  test('shown bounds are cut to the element’s page', () => {
    expect(shownBounds(paper, hanging)).toEqual({ x: -10, y: 150, width: 20, height: 50 });
    expect(shownBounds(paper, { ...hanging, x: 60, y: 10, height: 20 })).toBeNull();
  });
});
