import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import type { Paper } from '@/lib/canvas/paper';
import { createHarness } from './harness';

const base = { stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const box = (id: string, y: number): CanvasElement => ({ id, type: 'rectangle', x: -10, y, width: 20, height: 20, ...base });
// Two pages, 200 tall with a gap of 10: page one starts at y = 0, page two at y = 210.
const paper: Paper = { gap: 10, pages: [{ id: 'p1', width: 100, height: 200 }, { id: 'p2', width: 100, height: 200 }] };

function setup(extra: Paper = paper) {
  const h = createHarness();
  h.render({ ...createCanvasDocument(), paper: extra, elements: [box('on1', 20), box('on2', 240)] });
  h.render(); // the hook adopts a swapped-in document in an effect and shows it on the next render
  return h;
}
const state = (h: ReturnType<typeof setup>) => {
  const doc = h.editor.documentRef.current;
  return { pages: doc.paper!.pages.map(p => p.id), elements: doc.elements.map(e => [e.id, e.y]) };
};

describe('a notebook in the editor', () => {
  test('a whiteboard has no page controls', () => {
    const h = createHarness();
    h.render({ ...createCanvasDocument() });
    expect(h.editor.pages).toBeNull();
  });
  test('reports how many pages it has and which is in view', () => {
    const h = setup();
    expect(h.editor.pages).toMatchObject({ count: 2, index: 0 });
  });
  test('adding a page appends one, publishes it, and one undo takes it away', () => {
    const h = setup();
    h.editor.pages!.add(); h.render();
    expect(state(h).pages).toEqual(['p1', 'p2', 'p3']);
    expect(h.published.at(-1)!.paper!.pages).toHaveLength(3);
    h.editor.history(); h.render();
    expect(state(h).pages).toEqual(['p1', 'p2']);
    h.editor.history(true); h.render();
    expect(state(h).pages).toEqual(['p1', 'p2', 'p3']);
  });
  test('removing a page takes its ink and moves the pages below up, and undo puts all of it back', () => {
    const h = setup();
    h.editor.pages!.remove(0); h.render();
    expect(state(h)).toEqual({ pages: ['p2'], elements: [['on2', 30]] });
    h.editor.history(); h.render();
    expect(state(h)).toEqual({ pages: ['p1', 'p2'], elements: [['on1', 20], ['on2', 240]] });
  });
  test('the last page cannot be removed', () => {
    const h = setup({ pages: [paper.pages[0]] });
    const before = h.editor.documentRef.current;
    h.editor.pages!.remove(0); h.render();
    expect(h.editor.documentRef.current).toBe(before);
    expect(h.published).toHaveLength(0);
  });
  test('removing a page that is not there does nothing', () => {
    const h = setup();
    h.editor.pages!.remove(7); h.render();
    expect(h.published).toHaveLength(0);
  });
  test('removal clears a selection that pointed at the removed ink', () => {
    const h = setup();
    h.editor.keyDown({ key: 'a', code: '', metaKey: true, ctrlKey: false, shiftKey: false, altKey: false, target: { closest: () => null }, preventDefault: () => {}, stopPropagation: () => {}, defaultPrevented: false } as never);
    h.render();
    expect(h.editor.selection).toHaveLength(2);
    h.editor.pages!.remove(1); h.render();
    expect(h.editor.selection).toEqual([]);
  });
});
