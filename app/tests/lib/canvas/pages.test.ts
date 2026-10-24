import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasDocument, type CanvasElement } from '../../../src/lib/canvas/model';
import { addPage, removePage } from '../../../src/lib/canvas/pages';
import { createPaper, pageRects, pageView, type Paper } from '../../../src/lib/canvas/paper';
import { exportBounds, paperSvg, templateMarks } from '../../../src/lib/canvas/renderPaper';
import { exportCanvasSvg } from '../../../src/lib/canvas/renderSvg';
import { parseCanvasDocument, serializeCanvasDocument } from '../../../src/lib/canvas/model';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const box = (id: string, x: number, y: number): CanvasElement => ({ id, type: 'rectangle', x, y, width: 20, height: 20, ...base });
const notebook = (paper: Paper, elements: CanvasElement[] = []): CanvasDocument => ({ ...createCanvasDocument(), paper, elements });
const three: Paper = { gap: 10, pages: [{ id: 'p1', width: 100, height: 200 }, { id: 'p2', width: 100, height: 200 }, { id: 'p3', width: 100, height: 200 }] };

describe('addPage', () => {
  test('appends a page like the last and keeps every element where it is', () => {
    const doc = notebook(createPaper('grid'), [box('a', 0, 0)]);
    const next = addPage(doc);
    expect(next.paper!.pages).toHaveLength(2);
    expect(next.elements).toBe(doc.elements);
    expect(pageRects(next.paper!)[1].template).toBe('grid');
  });
  test('a whiteboard has nowhere to add one', () => {
    const doc = createCanvasDocument();
    expect(addPage(doc)).toBe(doc);
  });
});

describe('removePage', () => {
  // Pages are 200 tall with a gap of 10, so page n starts at y = 210 * (n - 1).
  const doc = notebook(three, [box('on1', 0, 20), box('on2', 0, 230), box('on3', 0, 440)]);

  test('takes the page and its elements, and moves the pages below up into the space', () => {
    const next = removePage(doc, 'p2');
    expect(next.paper!.pages.map(p => p.id)).toEqual(['p1', 'p3']);
    expect(next.elements.map(e => [e.id, e.y])).toEqual([['on1', 20], ['on3', 230]]);
    // The moved element still sits on the page that now starts where p2 did.
    expect(pageRects(next.paper!)[1].y).toBe(210);
  });
  test('removing the first page moves everything up', () => {
    const next = removePage(doc, 'p1');
    expect(next.elements.map(e => [e.id, e.y])).toEqual([['on2', 20], ['on3', 230]]);
  });
  test('removing the last page leaves the others alone', () => {
    const next = removePage(doc, 'p3');
    expect(next.elements.map(e => [e.id, e.y])).toEqual([['on1', 20], ['on2', 230]]);
  });
  test('a notebook keeps at least one page, and an unknown page is ignored', () => {
    const single = notebook(createPaper('blank'));
    expect(removePage(single, 'p1')).toBe(single);
    expect(removePage(doc, 'nope')).toBe(doc);
  });
  test('the result is still a valid canvas', () => {
    expect(() => parseCanvasDocument(serializeCanvasDocument(removePage(doc, 'p2')))).not.toThrow();
  });
});

describe('pageView', () => {
  const size = { width: 800, height: 600 };
  test('fits the widest page to the width, with a margin, up to 140%', () => {
    expect(pageView(three, { width: 148, height: 600 })).toEqual({ x: 74, y: 24, zoom: 1 });
    expect(pageView(three, size).zoom).toBe(1.4);
  });
  test('centres on x and puts the page top near the top of the screen', () => {
    const view = pageView(three, size, 2, 0.5);
    expect(view).toEqual({ x: 400, y: 24 - 420 * 0.5, zoom: 0.5 });
  });
  test('an out-of-range page index clamps', () => {
    expect(pageView(three, size, 99, 1).y).toBe(24 - 420);
    expect(pageView(three, size, -3, 1).y).toBe(24);
  });
});

describe('page templates', () => {
  const rect = (template: 'blank' | 'grid' | 'ruled' | 'dotted') => ({ ...pageRects({ pages: [{ id: 'p', width: 140, height: 280, template }] })[0] });

  test('blank paper has no marks', () => {
    expect(templateMarks(rect('blank'))).toEqual({ lines: [], dots: [] });
  });
  test('grid paper has a line every 14 points each way, inside the page', () => {
    const { lines, dots } = templateMarks(rect('grid'));
    expect(dots).toEqual([]);
    expect(lines.length / 4).toBe(21 + 11);
    expect(lines.slice(0, 4)).toEqual([-70, 0, 70, 0]);
  });
  test('ruled paper leaves a blank top margin and has horizontal lines only', () => {
    const { lines } = templateMarks(rect('ruled'));
    expect(lines.slice(0, 4)).toEqual([-70, 60, 70, 60]);
    expect(lines.every((v, i) => i % 4 !== 1 || v >= 60)).toBe(true);
    for (let i = 0; i < lines.length; i += 4) expect(lines[i + 1]).toBe(lines[i + 3]);
  });
  test('dotted paper has dots on the grid and no lines', () => {
    const { lines, dots } = templateMarks(rect('dotted'));
    expect(lines).toEqual([]);
    expect(dots.length / 2).toBe(11 * 21);
  });
  test('only the visible region is generated', () => {
    const all = templateMarks(rect('grid'));
    const part = templateMarks(rect('grid'), { x: -70, y: 0, width: 28, height: 28 });
    expect(part.lines.length).toBeLessThan(all.lines.length);
    expect(part.lines.length).toBeGreaterThan(0);
  });
  test('a pattern too fine to read is left out when zoomed far out', () => {
    expect(templateMarks(rect('grid'), undefined, 0.2).lines).toEqual([]);
  });
});

describe('notebook export', () => {
  test('bounds cover the pages and anything drawn off them', () => {
    const doc = notebook(three, [box('far', 400, 50)]);
    expect(exportBounds(doc)).toEqual({ x: -50, y: 0, width: 470, height: 620 });
  });
  test('a whiteboard exports around its elements, and an empty one has nothing to export', () => {
    expect(exportBounds(createCanvasDocument())).toBeNull();
    expect(exportBounds({ ...createCanvasDocument(), elements: [box('a', 5, 6)] })).toMatchObject({ x: 5, y: 6 });
  });
  test('the SVG has a white sheet for each page, under the elements', () => {
    const svg = exportCanvasSvg(notebook(createPaper('grid'), [box('a', 0, 0)]), () => 0);
    expect((svg.match(/fill="#ffffff"\/>/g) ?? []).length).toBeGreaterThanOrEqual(2);
    expect(svg.indexOf('stroke="#cfd8ea"')).toBeGreaterThan(-1);
    expect(svg.indexOf('stroke="#cfd8ea"')).toBeLessThan(svg.indexOf('<g transform'));
    expect(paperSvg(createPaper('blank'))).not.toContain('<path');
  });
});
