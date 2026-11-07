import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import { A4, pageView, zoomLimits, type CanvasKind, type Paper } from '@/lib/canvas/paper';
import { createHarness, key, pointer, type Harness } from './harness';

const base = { stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
// Two A4 pages: page one spans world x -297.5..297.5, y 0..842; page two y 866..1708.
const paper: Paper = { pages: [{ id: 'p1', ...A4 }, { id: 'p2', ...A4 }] };
const size = { width: 1000, height: 800 };
// The opening fit is 140%, centred: page one's left edge is at screen x 83.5, its top at y 24.
const fit = pageView(paper, size);
const block: CanvasElement = { id: 'block', type: 'rectangle', x: 200, y: 100, width: 60, height: 40, ...base };

function setup(kind: CanvasKind = 'notebook', document: Partial<CanvasDocument> = {}) {
  return createHarness({ kind, size }, { ...createCanvasDocument(), paper, ...document });
}
const view = (h: Harness) => h.editor.documentRef.current.viewport;
const elements = (h: Harness) => h.editor.documentRef.current.elements;
function drag(h: Harness, type: string, from: [number, number], to: [number, number], extra: Record<string, unknown> = {}) {
  h.editor.pointerDown(pointer(1, type, ...from, extra));
  h.editor.pointerMove(pointer(1, type, (from[0] + to[0]) / 2, (from[1] + to[1]) / 2, { buttons: 1, ...extra }));
  h.editor.pointerMove(pointer(1, type, ...to, { buttons: 1, ...extra }));
  h.editor.pointerUp(pointer(1, type, ...to, extra));
  h.render();
}
function useTool(h: Harness, tool: Parameters<Harness['editor']['setTool']>[0]) { h.editor.setTool(tool); h.render(); }

describe('a notebook in the editor', () => {
  test('opens fitted to its page width', () => {
    const h = setup();
    expect(view(h)).toMatchObject(fit);
    expect(h.editor.notebook).toBe(true);
  });
  test('a stroke started on the desk draws nothing, even when the pen then crosses onto the page', () => {
    const h = setup();
    useTool(h, 'freedraw');
    drag(h, 'pen', [40, 100], [400, 200]);
    expect(elements(h)).toEqual([]);
    expect(h.published).toHaveLength(0);
  });
  test('a stroke started on a page draws', () => {
    const h = setup();
    useTool(h, 'freedraw');
    drag(h, 'pen', [400, 100], [500, 200]);
    expect(elements(h).map(e => e.type)).toEqual(['freedraw']);
  });
  test('a shape cannot start in the gap between pages', () => {
    const h = setup();
    useTool(h, 'rectangle');
    // World y 854 is in the 24-point gap.
    drag(h, 'mouse', [500, 24 + 854 * fit.zoom], [600, 24 + 950 * fit.zoom]);
    expect(elements(h)).toEqual([]);
  });
  test('new text starts only on a page', () => {
    const h = setup();
    useTool(h, 'text');
    h.editor.pointerDown(pointer(1, 'mouse', 40, 100)); h.render();
    expect(h.editor.textDraft).toBeNull();
    h.editor.pointerDown(pointer(2, 'mouse', 400, 100)); h.render();
    expect(h.editor.textDraft).not.toBeNull();
  });
  test('the eraser and selection still work from the desk', () => {
    const h = setup('notebook', { elements: [block] });
    drag(h, 'mouse', [20, 20], [916, 400]);
    expect(h.editor.selection).toEqual(['block']);
  });
  test('a move cannot leave the selection’s centre on the desk', () => {
    const h = setup('notebook', { elements: [block] });
    const at = (x: number, y: number): [number, number] => [x * fit.zoom + fit.x, y * fit.zoom + fit.y];
    // An unfilled rectangle is picked by its outline, here between the left edge's handles.
    drag(h, 'mouse', at(200, 130), at(200, 130));
    drag(h, 'mouse', at(200, 130), at(870, 130));
    const moved = elements(h)[0];
    expect(moved.x + moved.width / 2).toBe(297.5);
    expect(moved.y).toBe(100);
  });
  test('arrow-key nudges stop at the page edge', () => {
    const h = setup('notebook', { elements: [{ ...block, x: 260 }] });
    h.editor.keyDown(key('a', { meta: true })); h.render();
    for (let i = 0; i < 5; i++) { h.editor.keyDown(key('ArrowRight', { shift: true })); h.render(); }
    expect(elements(h)[0].x + 30).toBe(297.5);
  });
  test('a duplicate that would hang its centre off the page is pulled back onto it', () => {
    const h = setup('notebook', { elements: [{ ...block, x: 260 }] });
    h.editor.keyDown(key('a', { meta: true })); h.render();
    h.editor.keyDown(key('d', { meta: true })); h.render();
    const copy = elements(h)[1];
    expect(copy.x + copy.width / 2).toBe(297.5);
    expect(copy.y).toBe(124);
  });
  test('images go onto the page in view, fitted inside it', () => {
    const h = setup();
    h.editor.insertImages([{ dataUrl: 'data:image/png;base64,fixture', width: 4000, height: 1000, name: 'wide' }]); h.render();
    const [image] = elements(h);
    expect(image.width).toBeLessThanOrEqual(A4.width);
    expect(image.x).toBeGreaterThanOrEqual(-297.5);
    expect(image.x + image.width).toBeLessThanOrEqual(297.5);
  });
  test('panning keeps the page column centred and the pages in view', () => {
    const h = setup();
    useTool(h, 'hand');
    drag(h, 'mouse', [500, 400], [900, 3000]);
    expect(view(h).x).toBe(500);
    expect(view(h).y).toBe(0.4 * size.height);
  });
  test('the wheel scrolls vertically within the overscroll and zooms only to the limits', () => {
    const h = setup();
    h.wheel({ deltaY: -5000 });
    expect(view(h).y).toBe(0.4 * size.height);
    h.wheel({ deltaY: 100000 });
    expect(view(h).y + 1708 * view(h).zoom).toBeCloseTo(0.6 * size.height);
    h.wheel({ deltaY: 1000, ctrlKey: true });
    expect(view(h).zoom).toBeCloseTo(zoomLimits(paper, size).min);
    h.render();
    expect(h.editor.canZoomOut).toBe(false);
    expect(h.editor.canZoomIn).toBe(true);
  });
  test('zooming out with the buttons stops at the notebook limit', () => {
    const h = setup();
    for (let i = 0; i < 20; i++) h.editor.zoom(1 / 1.2);
    expect(view(h).zoom).toBeCloseTo(zoomLimits(paper, size).min);
  });
  test('a saved camera off the pages is brought back when it opens, and again after a resize', () => {
    const h = setup('notebook', { viewport: { x: 4000, y: -9000, zoom: 1 } });
    expect(view(h).x).toBe(500);
    expect(view(h).y + 1708).toBeCloseTo(0.6 * size.height);
    h.resize({ width: 900, height: 800 });
    expect(view(h).x).toBe(450);
    // Narrower than the page at this zoom: it may pan only until an edge is 24px in.
    h.resize({ width: 500, height: 800 });
    expect(view(h).x).toBe(24 + 297.5);
  });
});

describe('a canvas holding paper is a whiteboard', () => {
  test('it opens at its own camera, pans anywhere and draws anywhere', () => {
    const h = setup('canvas');
    expect(h.editor.notebook).toBe(false);
    expect(view(h)).toEqual({ x: 0, y: 0, zoom: 1 });
    useTool(h, 'hand');
    drag(h, 'mouse', [500, 400], [900, 3000]);
    expect(view(h)).toMatchObject({ x: 400, y: 2600, zoom: 1 });
    useTool(h, 'freedraw');
    drag(h, 'pen', [40, 100], [60, 120]);
    expect(elements(h)).toHaveLength(1);
  });
  test('its zoom keeps the whiteboard range and moves are not held to a page', () => {
    const h = setup('canvas', { elements: [block] });
    for (let i = 0; i < 30; i++) h.editor.zoom(1 / 1.2);
    expect(view(h).zoom).toBe(0.1);
    h.editor.keyDown(key('a', { meta: true })); h.render();
    for (let i = 0; i < 20; i++) { h.editor.keyDown(key('ArrowRight', { shift: true })); h.render(); }
    expect(elements(h)[0].x).toBe(400);
  });
});
