import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { createWhiteboardDocument, type WhiteboardDocument, type WhiteboardElement } from '../../../src/lib/whiteboard/model';
import type { Paper } from '../../../src/lib/whiteboard/paper';
import { exportWhiteboardPng, paintWhiteboard, renderPagePng } from '../../../src/lib/whiteboard/render';
import { exportWhiteboardSvg } from '../../../src/lib/whiteboard/renderSvg';
import { rasterizeRegion } from '../../../src/lib/whiteboard/region';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const box = (id: string, x: number, y: number, width = 20, height = 20): WhiteboardElement => ({ id, type: 'rectangle', x, y, width, height, ...base });
// Page one spans x -50..50, y 0..200; page two y 210..410.
const paper: Paper = { gap: 10, pages: [{ id: 'p1', width: 100, height: 200 }, { id: 'p2', width: 100, height: 200 }] };
/** Its centre (x 70) is on page one's band, so page one owns it, and 30 of its 60 units hang off that page's right edge. */
const overhang = box('over', 40, 50, 60, 20);
const doc = (elements: WhiteboardElement[], withPaper = true): WhiteboardDocument => ({ ...createWhiteboardDocument(), ...(withPaper ? { paper } : {}), elements });
const measure = () => 0;

/** A 2D context that records every call by name and its arguments, and accepts any property. */
function recordingContext(calls: [string, unknown[]][]) {
  return new Proxy({}, {
    get: (_target, name) => name === 'getTransform' ? () => ({ a: 1 }) : (...args: unknown[]) => { calls.push([String(name), args]); },
    set: () => true,
  }) as unknown as CanvasRenderingContext2D;
}

let calls: [string, unknown[]][] = [], canvas: { width: number; height: number };
const saved = { window: globalThis.window, Path2D: (globalThis as { Path2D?: unknown }).Path2D };
beforeEach(() => {
  calls = [];
  canvas = Object.assign({ width: 0, height: 0 }, { getContext: () => recordingContext(calls), toBlob: (done: (blob: Blob) => void) => done(new Blob(['png'])) });
  Object.assign(globalThis, {
    window: { document: { createElement: () => canvas } },
    Path2D: class { roundRect() {} ellipse() {} moveTo() {} lineTo() {} quadraticCurveTo() {} bezierCurveTo() {} closePath() {} arc() {} },
  });
});
afterEach(() => { Object.assign(globalThis, saved); });
/** Each `clip` with the rectangle set just before it. */
const clips = () => calls.flatMap(([name], i) => name === 'clip' && calls[i - 1]?.[0] === 'rect' ? [calls[i - 1][1]] : []);

describe('on screen', () => {
  const paint = (document: WhiteboardDocument, notebook: boolean) => {
    calls = [];
    paintWhiteboard(recordingContext(calls), document, { width: 400, height: 300, pixelRatio: 1, grid: true, selected: new Set(), selectionColor: '#5e6ad2', notebook });
    return calls;
  };
  test('a whiteboard holding paper paints exactly as the same whiteboard without it, grid included', () => {
    expect(paint(doc([overhang]), false)).toEqual(paint(doc([overhang], false), false));
  });
  test('a notebook paints its sheets and clips its ink to them, with no grid', () => {
    const painted = paint(doc([overhang]), true);
    expect(clips()).toContainEqual([-50, 0, 100, 200]);
    expect(painted).not.toEqual(paint(doc([overhang], false), false));
  });
});

describe('PNG export', () => {
  test('a notebook covers only its pages and clips each element to its own', async () => {
    await exportWhiteboardPng(doc([overhang]), { notebook: true });
    expect(canvas.width).toBe((100 + 64) * 2);
    expect(clips()).toEqual([[-50, 0, 100, 200]]);
  });
  test('a whiteboard holding paper exports exactly as the same whiteboard without it', async () => {
    await exportWhiteboardPng(doc([overhang]));
    const withPaper = { width: canvas.width, calls };
    calls = [];
    await exportWhiteboardPng(doc([overhang], false));
    expect(withPaper.width).toBe((60 + 64) * 2);
    expect(withPaper.calls).toEqual(calls);
  });
  test('a whiteboard draws without a clip', async () => {
    await exportWhiteboardPng(doc([overhang], false), { notebook: true });
    expect(canvas.width).toBe((60 + 64) * 2);
    expect(calls.some(([name]) => name === 'clip')).toBe(false);
  });
  test('a run of elements on one page shares a clip, and a change of page opens a new one', async () => {
    await exportWhiteboardPng(doc([box('a', 0, 10), box('b', 0, 40), box('c', 0, 250), box('d', 0, 60)]), { notebook: true });
    expect(clips()).toEqual([[-50, 0, 100, 200], [-50, 210, 100, 200], [-50, 0, 100, 200]]);
  });
  test('a page picture leaves out ink from the page below that hangs onto it', async () => {
    const reaching = box('reach', 0, 190, 20, 40); // centre y 210: page two's
    await renderPagePng(doc([reaching]), 'p1', 2);
    expect(calls.filter(([name]) => name === 'translate').length).toBe(1);
    calls = [];
    await renderPagePng(doc([reaching]), 'p2', 2);
    expect(calls.filter(([name]) => name === 'translate').length).toBe(2);
  });
});

describe('SVG export', () => {
  test('a notebook has a clip path per page and wraps each run of elements in its page clip', () => {
    const svg = exportWhiteboardSvg(doc([overhang, box('two', 0, 250)]), measure, { notebook: true });
    expect(svg).toContain('viewBox="-82 -32 164 474"');
    expect(svg).toContain('<clipPath id="oculus-page-0"><rect x="-50" y="0" width="100" height="200"/></clipPath>');
    expect(svg).toContain('<clipPath id="oculus-page-1"><rect x="-50" y="210" width="100" height="200"/></clipPath>');
    expect(svg).toMatch(/<g clip-path="url\(#oculus-page-0\)"><g transform="translate\(40,50\)">.*<\/g><\/g><g clip-path="url\(#oculus-page-1\)"><g transform="translate\(0,250\)">/);
  });
  test('a whiteboard holding paper exports exactly as the same whiteboard without it: no pages, no clips', () => {
    const svg = exportWhiteboardSvg(doc([overhang]), measure);
    expect(svg).toBe(exportWhiteboardSvg(doc([overhang], false), measure));
    expect(svg).not.toContain('clip');
  });
  test('a whiteboard is unchanged by the notebook flag', () => {
    expect(exportWhiteboardSvg(doc([overhang], false), measure, { notebook: true })).toBe(exportWhiteboardSvg(doc([overhang], false), measure));
  });
});

describe('area captures', () => {
  test('in a notebook the captured ink is clipped to its page; on a whiteboard it is not', async () => {
    const region = { kind: 'capture' as const, bounds: { x: 0, y: 0, width: 120, height: 120 }, snapshot: doc([overhang]) };
    await rasterizeRegion(region, true);
    expect(clips()).toEqual([[-50, 0, 100, 200]]);
    calls = [];
    await rasterizeRegion(region);
    expect(clips()).toEqual([]);
  });
  test('a whiteboard capture holding paper is the same as one without it: no sheets painted', async () => {
    const region = { kind: 'capture' as const, bounds: { x: 0, y: 0, width: 120, height: 120 } };
    await rasterizeRegion({ ...region, snapshot: doc([overhang]) });
    const withPaper = calls;
    calls = [];
    await rasterizeRegion({ ...region, snapshot: doc([overhang], false) });
    expect(withPaper).toEqual(calls);
  });
});
