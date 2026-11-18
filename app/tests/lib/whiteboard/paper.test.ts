import { describe, expect, test } from 'bun:test';
import type { CanvasElement } from '../../../src/lib/canvas/model';
import { reorderElements } from '../../../src/lib/canvas/layers';
import {
  A4, createPaper, nextPageId, pageAt, pageOfElement, pageRects, paperBounds, paperProblem, selectableElements, withAddedPage,
  type Paper,
} from '../../../src/lib/canvas/paper';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const box = (id: string, extra: Partial<CanvasElement> = {}): CanvasElement => ({ id, type: 'rectangle', x: 0, y: 0, width: 50, height: 50, ...base, ...extra });

describe('pageRects', () => {
  const paper: Paper = { gap: 10, pages: [{ id: 'a', width: 600, height: 800 }, { id: 'b', width: 400, height: 500, template: 'grid' }] };

  test('pages stack down the canvas, each centred on x = 0', () => {
    expect(pageRects(paper)).toEqual([
      { id: 'a', index: 0, x: -300, y: 0, width: 600, height: 800, template: 'blank' },
      { id: 'b', index: 1, x: -200, y: 810, width: 400, height: 500, template: 'grid' },
    ]);
  });
  test('the gap defaults to 24 units', () => {
    expect(pageRects({ pages: paper.pages })[1].y).toBe(824);
  });
  test('bounds wrap every page', () => {
    expect(paperBounds(paper)).toEqual({ x: -300, y: 0, width: 600, height: 1310 });
  });
  test('a point belongs to the page above it when it falls in a gap or past the end', () => {
    expect(pageAt(paper, { x: 0, y: 100 }).id).toBe('a');
    expect(pageAt(paper, { x: 0, y: 804 }).id).toBe('a');
    expect(pageAt(paper, { x: 0, y: 808 }).id).toBe('b');
    expect(pageAt(paper, { x: 0, y: 99999 }).id).toBe('b');
    expect(pageAt(paper, { x: 0, y: -50 }).id).toBe('a');
  });
  test('an element belongs to the page holding the centre of its bounds', () => {
    expect(pageOfElement(paper, { x: -20, y: 900, width: 40, height: 40 }).id).toBe('b');
    expect(pageOfElement(paper, { x: -20, y: 790, width: 40, height: 4 }).id).toBe('a');
  });
});

describe('building papers', () => {
  test('a new paper is one A4 page, portrait or landscape', () => {
    expect(createPaper('grid').pages).toEqual([{ id: 'p1', width: A4.width, height: A4.height, template: 'grid' }]);
    expect(createPaper('blank', 'landscape').pages[0]).toMatchObject({ width: A4.height, height: A4.width });
  });
  test('an added page copies the last one and takes a fresh id', () => {
    const paper = withAddedPage(withAddedPage(createPaper('ruled')));
    expect(paper.pages.map(p => p.id)).toEqual(['p1', 'p2', 'p3']);
    expect(paper.pages[2]).toMatchObject({ width: A4.width, template: 'ruled' });
  });
  test('ids never reuse the number of a deleted page', () => {
    const paper: Paper = { pages: [{ id: 'p1', width: 1, height: 1 }, { id: 'p4', width: 1, height: 1 }, { id: 'cover', width: 1, height: 1 }] };
    expect(nextPageId(paper)).toBe('p5');
  });
});

describe('paperProblem', () => {
  const ok = { pages: [{ id: 'p1', width: 595, height: 842 }] };
  test('accepts a paper and keeps no opinion about extra fields', () => {
    expect(paperProblem(ok)).toBeNull();
    expect(paperProblem({ ...ok, note: 1, pages: [{ ...ok.pages[0], note: 2 }] })).toBeNull();
  });
  test.each([
    ['null', null], ['array', []], ['no pages', {}], ['empty pages', { pages: [] }],
    ['duplicate ids', { pages: [ok.pages[0], ok.pages[0]] }],
    ['zero width', { pages: [{ id: 'p', width: 0, height: 1 }] }],
    ['bad template', { pages: [{ id: 'p', width: 1, height: 1, template: 'lined' }] }],
    ['negative gap', { gap: -1, pages: ok.pages }],
  ])('rejects %s', (_name, value) => {
    expect(paperProblem(value)).not.toBeNull();
  });
});

describe('locked elements', () => {
  const page = box('page', { locked: true, type: 'image' });
  const ink = box('ink');

  test('are left out of what the pointer can pick', () => {
    expect(selectableElements([page, ink]).map(e => e.id)).toEqual(['ink']);
  });
  test('a whiteboard keeps its own array, so its cached geometry is reused', () => {
    const elements = [ink, box('other')];
    expect(selectableElements(elements)).toBe(elements);
  });
  test('the filtered array is the same one every time', () => {
    const elements = [page, ink];
    expect(selectableElements(elements)).toBe(selectableElements(elements));
  });
  test('layer moves never send ink beneath a locked page', () => {
    const a = box('a'), b = box('b');
    expect(reorderElements([page, a, b], new Set(['b']), 'back').map(e => e.id)).toEqual(['page', 'b', 'a']);
    expect(reorderElements([page, a, b], new Set(['a']), 'backward').map(e => e.id)).toEqual(['page', 'a', 'b']);
  });
  test('a move that changes nothing returns the same array', () => {
    const elements = [page, box('a'), box('b')];
    expect(reorderElements(elements, new Set(['a']), 'back')).toBe(elements);
  });
});
