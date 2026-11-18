import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, parseCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import type { Paper } from '@/lib/canvas/paper';
import { applyCanvasTransaction, diffCanvasContent, sameValue } from '@/lib/canvas/collaboration';
import { createHarness, key } from './harness';

const base = { stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const box = (id: string, y: number): CanvasElement => ({ id, type: 'rectangle', x: -10, y, width: 20, height: 20, ...base });
// Two pages, 200 tall with a gap of 10: page one starts at y = 0, page two at y = 210.
const paper: Paper = { gap: 10, pages: [{ id: 'p1', width: 100, height: 200 }, { id: 'p2', width: 100, height: 200 }] };

function setup(extra: Paper = paper) {
  const h = createHarness({ kind: 'notebook' });
  h.render({ ...createCanvasDocument(), paper: extra, elements: [box('on1', 20), box('on2', 240)] });
  h.render(); // the hook adopts a swapped-in document in an effect and shows it on the next render
  return h;
}
const state = (h: ReturnType<typeof setup>) => {
  const doc = h.editor.documentRef.current;
  return { pages: doc.paper!.pages.map(p => p.id), elements: doc.elements.map(e => [e.id, e.y]) };
};

describe('a notebook in the editor', () => {
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
  test('removal drops the removed ink from the selection and keeps the rest', () => {
    const h = setup();
    h.editor.keyDown(key('a', { meta: true })); h.render();
    expect(h.editor.selection).toHaveLength(2);
    h.editor.pages!.remove(1); h.render();
    expect(h.editor.selection).toEqual(['on1']);
  });
});

/** The edit the last page operation sent, applied to the document before it as the authority would. */
function replayed(before: CanvasDocument, after: CanvasDocument) {
  const transaction = diffCanvasContent(before, after);
  expect(transaction.paper).toBeDefined();
  return sameValue({ ...applyCanvasTransaction(before, transaction), viewport: null }, { ...after, viewport: null });
}

describe('page tools in a notebook', () => {
  const ops: [string, (h: ReturnType<typeof setup>) => void, string[]][] = [
    ['adds a page before', h => h.editor.pages!.insert(1, 'before'), ['p1', 'p3', 'p2']],
    ['adds a page after', h => h.editor.pages!.insert(1, 'after'), ['p1', 'p2', 'p3']],
    ['duplicates a page', h => h.editor.pages!.duplicate(0), ['p1', 'p3', 'p2']],
    ['moves a page', h => h.editor.pages!.move(1, 0), ['p2', 'p1']],
    ['changes a template', h => h.editor.pages!.setTemplate(0, 'ruled'), ['p1', 'p2']],
    ['changes a size', h => h.editor.pages!.setSize(0, { width: 200, height: 100 }), ['p1', 'p2']],
    ['inserts PDF pages', h => h.editor.pages!.insertPdfPages(0, [{ width: 50, height: 50 }], [{ dataUrl: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=', width: 1, height: 1, name: 'page' }]), ['p1', 'p3', 'p2']],
  ];
  for (const [name, run, order] of ops) {
    test(`${name} as one undo step that a shared copy can apply`, () => {
      const h = setup(), before = h.editor.documentRef.current;
      run(h); h.render();
      expect(state(h).pages).toEqual(order);
      expect(h.published).toHaveLength(1);
      expect(replayed(before, h.published[0])).toBe(true);
      h.editor.history(); h.render();
      expect(sameValue({ ...h.editor.documentRef.current, viewport: null }, { ...before, viewport: null })).toBe(true);
      h.editor.history(true); h.render();
      expect(state(h).pages).toEqual(order);
    });
  }
  test('moving a page takes its ink along', () => {
    const h = setup();
    h.editor.pages!.move(1, 0); h.render();
    expect(state(h).elements).toEqual([['on1', 230], ['on2', 30]]);
  });
  test('the view goes to the page an operation made', () => {
    const tall: Paper = { gap: 10, pages: [{ id: 'p1', width: 100, height: 1000 }, { id: 'p2', width: 100, height: 1000 }] };
    const h = createHarness({ kind: 'notebook', size: { width: 400, height: 300 } }, { ...createCanvasDocument(), paper: tall });
    expect(h.editor.pages!.index).toBe(0);
    h.editor.pages!.insert(0, 'after'); h.render();
    expect(h.editor.pages!.index).toBe(1);
    h.editor.pages!.move(1, 2); h.render();
    expect(h.editor.pages!.index).toBe(2);
  });
  test('a duplicate keeps the original selected and its copies unselected', () => {
    const h = setup();
    h.editor.keyDown(key('a', { meta: true })); h.render();
    h.editor.pages!.duplicate(0); h.render();
    expect(h.editor.selection).toEqual(['on1', 'on2']);
  });
});

describe('a canvas holding paper', () => {
  test('has no pages and no page controls', () => {
    const h = createHarness({ kind: 'canvas' }, { ...createCanvasDocument(), paper, elements: [box('on1', 20)] });
    expect(h.editor.pages).toBeNull();
    expect(h.editor.notebook).toBe(false);
  });
  test('keeps its paper, untouched, through an edit and a save', () => {
    const loaded = parseCanvasDocument(serializeCanvasDocument({ ...createCanvasDocument(), paper, elements: [box('on1', 20)] }));
    const h = createHarness({ kind: 'canvas' }, loaded);
    h.editor.keyDown(key('a', { meta: true })); h.render();
    h.editor.keyDown(key('ArrowDown', { shift: true })); h.render();
    const saved = parseCanvasDocument(serializeCanvasDocument(h.published.at(-1)!));
    expect(saved.elements.map(e => e.y)).toEqual([30]);
    expect(saved.paper).toEqual(paper);
  });
  test('a notebook without paper has no page controls either', () => {
    const h = createHarness({ kind: 'notebook' });
    expect(h.editor.pages).toBeNull();
  });
});
