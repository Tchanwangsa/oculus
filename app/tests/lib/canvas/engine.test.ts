import { beforeAll, describe, expect, test } from 'bun:test';
import { initSync } from '../../../canvas-core/pkg/oculus_canvas_core';
import { getCanvasEngineStatus, hitTest, loadCanvasEngine, simplifyPoints, visibleElements } from '../../../src/lib/canvas/engine';
import { boundsIntersect, elementBounds, elementHitTest, simplifyPolyline } from '../../../src/lib/canvas/geometry';
import type { CanvasArrowhead, CanvasElement } from '../../../src/lib/canvas/model';

beforeAll(async () => {
  initSync({ module: await Bun.file(new URL('../../../canvas-core/pkg/oculus_canvas_core_bg.wasm', import.meta.url)).arrayBuffer() });
  await loadCanvasEngine();
});

const types: CanvasElement['type'][] = ['rectangle', 'ellipse', 'diamond', 'line', 'arrow', 'freedraw', 'text'];
const elements: CanvasElement[] = Array.from({ length: 280 }, (_, i) => ({
  id: String(i), type: types[i % types.length], x: (i % 20) * 200 - 1000, y: Math.floor(i / 20) * 160 - 400,
  width: i % 3 ? 100 : -100, height: 90, stroke: '#111111', fill: i % 2 ? 'transparent' : '#dddddd', strokeWidth: 1 + i % 4,
  style: 'clean', seed: i, ...(i % 7 >= 3 && i % 7 <= 5 ? { points: [{ x: 0, y: 0 }, { x: 30, y: 70 }, { x: 100, y: 90 }] } : {}),
  ...(i % 7 === 4 ? heads[i % heads.length] : {}), ...(i % 7 <= 2 && i % 5 === 1 ? { text: 'Label' } : {}),
}));
const arrowheads: CanvasArrowhead[] = ['none', 'arrow', 'triangle', 'dot', 'bar', 'diamond'];
function heads(i: number) { return { startArrowhead: arrowheads[i % 6], endArrowhead: arrowheads[(i * 5 + 1) % 6] }; }
const arrows: CanvasElement[] = arrowheads.flatMap((start, i) => (['none', 'arrow'] as const).map((end, j) => ({
  id: `arrow-${i}-${j}`, type: 'arrow' as const, x: i * 300, y: j * 300, width: 120, height: -60, stroke: '#111111', fill: 'transparent', strokeWidth: 1 + i % 3,
  style: 'clean' as const, seed: i, startArrowhead: start, endArrowhead: end, ...(i % 2 ? { points: [{ x: 0, y: 0 }, { x: 40, y: 10 }, { x: 120, y: -60 }] } : {}),
})));

describe('retained Rust canvas engine', () => {
  test('actually loads WebAssembly and matches reference hit tests across the scene', () => {
    expect(getCanvasEngineStatus()).toBe('rust');
    for (let y = -420; y < 1800; y += 47) for (let x = -1020; x < 2900; x += 53) {
      const point = { x, y };
      const expected = [...elements].reverse().find(el => elementHitTest(el, point, 4))?.id ?? null;
      expect(hitTest(elements, point, 4)).toBe(expected);
    }
  });
  test('matches the reference around start and end arrowheads, including ends without one', () => {
    for (const arrow of arrows) {
      const points = arrow.points ?? [{ x: 0, y: 0 }, { x: arrow.width, y: arrow.height }];
      for (const end of [points[0], points[points.length - 1]]) for (let dy = -24; dy <= 24; dy += 1.5) for (let dx = -24; dx <= 24; dx += 1.5) {
        const point = { x: arrow.x + end.x + dx, y: arrow.y + end.y + dy };
        expect(hitTest(arrows, point, 2)).toBe([...arrows].reverse().find(el => elementHitTest(el, point, 2))?.id ?? null);
      }
    }
    expect(hitTest(arrows, { x: 12, y: 1 }, 0)).toBeNull();
    expect(hitTest(arrows, { x: 612, y: 1 }, 0)).toBe('arrow-2-0');
  });
  test('a labelled hollow shape is hit inside, as in the reference', () => {
    const hollow: CanvasElement = { ...elements[0], id: 'hollow', type: 'rectangle', x: 0, y: 0, width: 100, height: 100, fill: 'transparent' };
    expect(hitTest([hollow], { x: 50, y: 50 }, 0)).toBeNull();
    for (const type of ['rectangle', 'ellipse', 'diamond'] as const) expect(hitTest([{ ...hollow, type, text: 'Label' }], { x: 50, y: 50 }, 0)).toBe('hollow');
  });
  test('culls with stroke padding and preserves paint order', () => {
    const bounds = { x: -560, y: -100, width: 800, height: 650 };
    const expected = elements.filter(el => {
      const b = elementBounds(el), pad = el.strokeWidth / 2 + (el.type === 'arrow' ? 16 : 0) + 2;
      return boundsIntersect({ x: b.x - pad, y: b.y - pad, width: b.width + pad * 2, height: b.height + pad * 2 }, bounds);
    });
    expect(visibleElements(elements, bounds)).toEqual(expected);
    expect(visibleElements([], bounds)).toEqual([]);
  });
  test('rebuilds edited scene geometry and safely evicts retained scenes', () => {
    for (let i = 0; i < 20; i++) {
      const changed = [{ ...elements[0], x: i * 100, y: 0, width: 50 }];
      expect(hitTest(changed, { x: i * 100 + 20, y: 20 }, 0)).toBe('0');
    }
    expect(hitTest(elements, { x: -1000, y: -400 }, 0)).toBe('0');
  });
  test('Rust stroke simplification matches the reference and preserves endpoints', () => {
    const points = Array.from({ length: 2000 }, (_, i) => ({ x: i / 10, y: Math.sin(i / 50) * 40 }));
    expect(simplifyPoints(points, 0.5)).toEqual(simplifyPolyline(points, 0.5));
    expect(simplifyPoints([], 1)).toEqual([]);
  });
});
