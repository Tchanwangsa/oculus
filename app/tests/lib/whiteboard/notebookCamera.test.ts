import { describe, expect, test } from 'bun:test';
import {
  A4, clampCamera, keepOnPage, pageAt, pageContaining, pageInView, pageLocator, pageView, visiblePageCentre, zoomLimits, type Paper,
} from '../../../src/lib/whiteboard/paper';

// Two A4 pages, 24 apart: page one spans y 0..842, page two 866..1708, both x -297.5..297.5.
const paper: Paper = { pages: [{ id: 'p1', ...A4 }, { id: 'p2', ...A4 }] };
const size = { width: 1000, height: 800 };
/** The page column's left and right edges and the paper's top and bottom on screen. */
const onScreen = (view: { x: number; y: number; zoom: number }) => ({
  left: -297.5 * view.zoom + view.x, right: 297.5 * view.zoom + view.x, top: view.y, bottom: 1708 * view.zoom + view.y,
});

describe('zoomLimits', () => {
  test('a notebook zooms out to 60% of its fitted view and in to 800%', () => {
    const narrow = { width: 500, height: 800 }, fit = (narrow.width - 48) / A4.width;
    expect(zoomLimits(paper, narrow)).toEqual({ min: 0.6 * fit, max: 8 });
  });
  test('a wide window, whose fitted view stops at 140%, still zooms out below it', () => {
    const wide = { width: 3000, height: 800 };
    expect(pageView(paper, wide).zoom).toBe(1.4);
    expect(zoomLimits(paper, wide).min).toBeCloseTo(0.84);
  });
  test('the widest page sets the fit', () => {
    const mixed: Paper = { pages: [{ id: 'a', width: 400, height: 500 }, { id: 'b', width: 952, height: 500 }] };
    expect(zoomLimits(mixed, size).min).toBeCloseTo(0.6);
  });
});

describe('clampCamera', () => {
  test('a page column narrower than the view is centred, so it cannot pan sideways', () => {
    const view = clampCamera(paper, size, { x: 900, y: 24, zoom: 1 });
    expect(view.x).toBe(500);
    expect(view.y).toBe(24);
  });
  test('the opening fit is already inside, and comes back as the same object', () => {
    const view = pageView(paper, size);
    expect(clampCamera(paper, size, view)).toBe(view);
  });
  test('zoomed in, it pans sideways only until the column edge is 24px inside the view', () => {
    const left = onScreen(clampCamera(paper, size, { x: 5000, y: 24, zoom: 3 }));
    expect(left.left).toBeCloseTo(24);
    const right = onScreen(clampCamera(paper, size, { x: -5000, y: 24, zoom: 3 }));
    expect(right.right).toBeCloseTo(size.width - 24);
    const between = { x: 450, y: 24, zoom: 3 };
    expect(clampCamera(paper, size, between)).toBe(between);
  });
  test('desk shows above the first page or below the last for at most 40% of the view', () => {
    expect(onScreen(clampCamera(paper, size, { x: 500, y: 5000, zoom: 1 })).top).toBeCloseTo(0.4 * size.height);
    expect(onScreen(clampCamera(paper, size, { x: 500, y: -50000, zoom: 1 })).bottom).toBeCloseTo(0.6 * size.height);
    const scrolled = { x: 500, y: -600, zoom: 1 };
    expect(clampCamera(paper, size, scrolled)).toBe(scrolled);
  });
  test('pages shorter than the allowed band sit in the middle of the view', () => {
    const tiny: Paper = { pages: [{ id: 'a', width: 595, height: 50 }] };
    const view = clampCamera(tiny, size, { x: 500, y: 700, zoom: 1 });
    expect(view.y + 25 * view.zoom).toBeCloseTo(size.height / 2);
  });
  test('a zoom outside the limits is brought in about the view centre', () => {
    const { min } = zoomLimits(paper, size);
    expect(clampCamera(paper, size, { x: 500, y: 100, zoom: 0.1 }).zoom).toBe(min);
    expect(clampCamera(paper, size, { x: 500, y: 100, zoom: 20 }).zoom).toBe(8);
  });
  test('before the view has a size nothing is clamped', () => {
    const view = { x: 9000, y: 9000, zoom: 0.1 };
    expect(clampCamera(paper, { width: 0, height: 0 }, view)).toBe(view);
  });
});

describe('page containment', () => {
  test('pageContaining finds the page under a point and nothing on the desk', () => {
    expect(pageContaining(paper, { x: 0, y: 10 })?.id).toBe('p1');
    expect(pageContaining(paper, { x: 297.5, y: 842 })?.id).toBe('p1');
    expect(pageContaining(paper, { x: 0, y: 900 })?.id).toBe('p2');
    expect(pageContaining(paper, { x: 0, y: 850 })).toBeNull();
    expect(pageContaining(paper, { x: 400, y: 10 })).toBeNull();
    expect(pageContaining(paper, { x: 0, y: -1 })).toBeNull();
  });
  test('pageAt, unlike it, ignores x and answers the nearest page', () => {
    expect(pageAt(paper, { x: 400, y: 10 }).id).toBe('p1');
  });
  test('pageLocator agrees with pageAt everywhere', () => {
    const many: Paper = { gap: 10, pages: Array.from({ length: 7 }, (_, i) => ({ id: `p${i}`, width: 100, height: 100 + i * 20 })) };
    const locate = pageLocator(many);
    for (let y = -50; y < 1600; y += 7) expect(locate({ x: 0, y }).id).toBe(pageAt(many, { x: 0, y }).id);
  });
  test('keepOnPage leaves a move that ends on a page and shortens one that would leave the pages', () => {
    expect(keepOnPage(paper, { x: 0, y: 100 }, { x: 0, y: 900 })).toEqual({ x: 0, y: 900 });
    expect(keepOnPage(paper, { x: 0, y: 100 }, { x: 500, y: 0 })).toEqual({ x: 297.5, y: 0 });
    expect(keepOnPage(paper, { x: 0, y: 100 }, { x: 0, y: -300 })).toEqual({ x: 0, y: -100 });
    // Into the gap between pages: back to the nearer page's edge.
    expect(keepOnPage(paper, { x: 0, y: 100 }, { x: 0, y: 745 })).toEqual({ x: 0, y: 742 });
  });
});

describe('the page in view', () => {
  test('is the page at the centre of the screen', () => {
    expect(pageInView(paper, pageView(paper, size, 1, 1), size).id).toBe('p2');
  });
  test('its visible part has a centre; off screen, the page centre stands in', () => {
    const view = { x: 500, y: 0, zoom: 1 };
    expect(visiblePageCentre({ id: 'p1', index: 0, x: -297.5, y: 0, width: 595, height: 842, template: 'blank' }, view, size)).toEqual({ x: 0, y: 400 });
    expect(visiblePageCentre({ id: 'p2', index: 1, x: -297.5, y: 866, width: 595, height: 842, template: 'blank' }, view, size)).toEqual({ x: 0, y: 1287 });
  });
});
