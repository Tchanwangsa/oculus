import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, parseCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasElement } from '../../../src/lib/canvas/model';
import { duplicatePage, insertPage, insertPdfPages, movePage, setPageSize, setPageTemplate } from '../../../src/lib/canvas/pages';
import { A4, pageRects, paperProblem, type Paper } from '../../../src/lib/canvas/paper';
import { elementPage } from '../../../src/lib/canvas/renderPaper';

const PNG = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=';
const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const box = (id: string, x: number, y: number, extra: Partial<CanvasElement> = {}): CanvasElement => ({ id, type: 'rectangle', x, y, width: 20, height: 20, ...base, ...extra });
const image = (id: string, y: number): CanvasElement => ({
  id, type: 'image', x: -50, y, width: 100, height: 200, dataUrl: PNG, naturalWidth: 10, naturalHeight: 20,
  stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0, locked: true,
});
// Pages are 100 x 200 with a gap of 10, so page n starts at y = 210 * (n - 1).
const three: Paper = { gap: 10, pages: [{ id: 'p1', width: 100, height: 200, template: 'grid' }, { id: 'p2', width: 100, height: 200 }, { id: 'p3', width: 100, height: 200, template: 'ruled' }] };
const notebook = (elements: CanvasElement[] = [], paper: Paper = three): CanvasDocument => ({ ...createCanvasDocument(), paper, elements });
const doc = notebook([image('bg2', 210), box('on1', 0, 20), box('on2', 0, 230), box('on3', 0, 440)]);

const ys = (document: CanvasDocument) => Object.fromEntries(document.elements.map(e => [e.id, e.y]));
const pages = (document: CanvasDocument) => document.paper!.pages.map(p => p.id);
const owner = (document: CanvasDocument, id: string) => elementPage(document.paper!, document.elements.find(e => e.id === id)!).id;
/** Every result must still be a document the app and the Rust authority accept. */
function valid(document: CanvasDocument) {
  expect(paperProblem(document.paper)).toBeNull();
  expect(new Set(document.elements.map(e => e.id)).size).toBe(document.elements.length);
  expect(() => parseCanvasDocument(serializeCanvasDocument(document))).not.toThrow();
}

describe('insertPage', () => {
  test('a page before another pushes it and everything below down by one page', () => {
    const next = insertPage(doc, 1);
    expect(pages(next)).toEqual(['p1', 'p4', 'p2', 'p3']);
    expect(ys(next)).toEqual({ bg2: 420, on1: 20, on2: 440, on3: 650 });
    expect(owner(next, 'on2')).toBe('p2');
    valid(next);
  });
  test('a new page copies its neighbour’s size and template, but nothing drawn on it', () => {
    const next = insertPage(doc, 3);
    expect(next.paper!.pages[3]).toEqual({ id: 'p4', width: 100, height: 200, template: 'ruled' });
    expect(next.elements).toEqual(doc.elements);
    expect(insertPage(doc, 0).paper!.pages[0]).toMatchObject({ template: 'grid' });
    expect(insertPage(doc, 0, { width: 300, height: 50 }).paper!.pages[0]).toEqual({ id: 'p4', width: 300, height: 50 });
  });
  test('an index outside the notebook, or a whiteboard, is left alone', () => {
    expect(insertPage(doc, 9)).toBe(doc);
    expect(insertPage(doc, -1)).toBe(doc);
    const board = createCanvasDocument();
    expect(insertPage(board, 0)).toBe(board);
  });
});

describe('duplicatePage', () => {
  test('copies the page after itself with clones of its elements under new ids, and moves the rest down', () => {
    const next = duplicatePage(doc, 1);
    expect(pages(next)).toEqual(['p1', 'p2', 'p4', 'p3']);
    expect(next.paper!.pages[2]).toMatchObject({ width: 100, height: 200 });
    const clones = next.elements.filter(e => !doc.elements.some(o => o.id === e.id));
    expect(clones.map(e => [e.type, e.y])).toEqual([['image', 420], ['rectangle', 440]]);
    expect(clones.every(e => owner(next, e.id) === 'p4')).toBe(true);
    expect(ys(next)).toMatchObject({ on1: 20, bg2: 210, on2: 230, on3: 650 });
    valid(next);
  });
  test('a locked backdrop stays locked and stays beneath the ink', () => {
    const next = duplicatePage(doc, 1);
    const copy = next.elements.find(e => e.type === 'image' && e.id !== 'bg2')!;
    expect(copy.locked).toBe(true);
    expect(next.elements.findIndex(e => e.id === copy.id)).toBe(1);
  });
  test('an empty page copies as an empty page', () => {
    const next = duplicatePage(doc, 2);
    expect(next.elements.length).toBe(doc.elements.length + 1);
    expect(duplicatePage(notebook(), 0).elements).toEqual([]);
  });
  test('an arrow bound inside the page keeps its binding on its copy; one bound elsewhere loses it', () => {
    const arrow = (id: string, target: string): CanvasElement => ({ ...box(id, -10, 60), type: 'arrow', points: [{ x: 0, y: 0 }, { x: 10, y: 10 }], startBinding: { elementId: target, mode: 'center' } });
    const next = duplicatePage(notebook([box('a', 0, 20), arrow('in', 'a'), arrow('out', 'on3'), box('on3', 0, 440)]), 0);
    const [boxCopy, inCopy, outCopy] = next.elements.slice(-3);
    expect(inCopy.startBinding?.elementId).toBe(boxCopy.id);
    expect(outCopy.startBinding).toBeUndefined();
  });
});

describe('movePage', () => {
  test('every page’s elements go with it', () => {
    const next = movePage(doc, 2, 0);
    expect(pages(next)).toEqual(['p3', 'p1', 'p2']);
    expect(ys(next)).toEqual({ bg2: 420, on1: 230, on2: 440, on3: 20 });
    for (const id of ['on1', 'on2', 'on3', 'bg2']) expect(owner(next, id)).toBe(owner(doc, id));
    valid(next);
  });
  test('pages of different heights land where the new order puts them', () => {
    const mixed = notebook([box('tall', 0, 100), box('short', 0, 320)], { gap: 10, pages: [{ id: 'a', width: 100, height: 300 }, { id: 'b', width: 100, height: 50 }] });
    const next = movePage(mixed, 0, 1);
    expect(pageRects(next.paper!).map(r => r.y)).toEqual([0, 60]);
    expect(ys(next)).toEqual({ short: 10, tall: 160 });
  });
  test('moving a page to where it is, or out of range, changes nothing', () => {
    expect(movePage(doc, 1, 1)).toBe(doc);
    expect(movePage(doc, 1, 3)).toBe(doc);
  });
});

describe('setPageTemplate', () => {
  test('changes that page only and moves nothing', () => {
    const next = setPageTemplate(doc, 1, 'dotted');
    expect(next.paper!.pages.map(p => p.template)).toEqual(['grid', 'dotted', 'ruled']);
    expect(next.elements).toBe(doc.elements);
    valid(next);
  });
  test('the template it already has is no change', () => {
    expect(setPageTemplate(doc, 0, 'grid')).toBe(doc);
    expect(setPageTemplate(doc, 1, 'blank')).toBe(doc);
  });
});

describe('setPageSize', () => {
  test('elements keep their offset from the page’s top centre and the pages below move by the change in height', () => {
    const next = setPageSize(doc, 0, { width: 300, height: 400 });
    expect(next.paper!.pages[0]).toMatchObject({ width: 300, height: 400 });
    expect(ys(next)).toEqual({ bg2: 410, on1: 20, on2: 430, on3: 640 });
    expect(next.elements.find(e => e.id === 'on1')!.x).toBe(0);
    valid(next);
  });
  test('an element whose centre would leave a smaller page is kept, pulled just onto it, and still its own', () => {
    const next = setPageSize(notebook([box('low', 30, 170)]), 0, { width: 40, height: 100 });
    const low = next.elements[0];
    expect(low.x + 10).toBe(20);
    expect(low.y + 10).toBe(100);
    expect(owner(next, 'low')).toBe('p1');
  });
  test('A4 landscape on a PDF page keeps its backdrop', () => {
    const pdf = notebook([image('bg', 0)], { pages: [{ id: 'p1', ...A4 }] });
    const next = setPageSize(pdf, 0, { width: A4.height, height: A4.width });
    expect(next.elements[0]).toMatchObject({ id: 'bg', locked: true });
    expect(setPageSize(pdf, 0, A4)).toBe(pdf);
  });
});

describe('insertPdfPages', () => {
  const picture = (name: string) => ({ dataUrl: PNG, width: 40, height: 60, name });
  test('adds a page per picture at its size, each with a locked backdrop filling it', () => {
    const next = insertPdfPages(doc, 1, [{ width: 400, height: 600 }, { width: 600, height: 400 }], [picture('a'), picture('b')]);
    expect(pages(next)).toEqual(['p1', 'p4', 'p5', 'p2', 'p3']);
    const rects = pageRects(next.paper!);
    const added = next.elements.filter(e => !doc.elements.some(o => o.id === e.id));
    expect(added.map(e => ({ x: e.x, y: e.y, width: e.width, height: e.height, locked: e.locked, sourceName: e.sourceName })))
      .toEqual([1, 2].map(i => ({ x: rects[i].x, y: rects[i].y, width: rects[i].width, height: rects[i].height, locked: true, sourceName: i === 1 ? 'a' : 'b' })));
    expect(ys(next)).toMatchObject({ on1: 20, on2: 230 + 1020, on3: 440 + 1020 });
    valid(next);
  });
  test('the new backdrops go beneath every bit of ink, above the old backdrops', () => {
    const next = insertPdfPages(doc, 3, [{ width: 100, height: 100 }], [picture('a')]);
    expect(next.elements.map(e => e.locked === true)).toEqual([true, true, false, false, false]);
  });
  test('a picture count that does not match the sizes is refused; none is no change', () => {
    expect(() => insertPdfPages(doc, 0, [{ width: 1, height: 1 }], [])).toThrow();
    expect(insertPdfPages(doc, 0, [], [])).toBe(doc);
  });
});
