import { describe, expect, test } from 'bun:test';
import { parseCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasElement, type CanvasPen } from '@/lib/canvas/model';
import { MARKERS, PENS, nibOutline, paintsIncrementally, penLine, strokeSamples } from '@/lib/canvas/pens';
import { exportCanvasSvg } from '@/lib/canvas/renderSvg';
import { DEFAULT_DRAWING_STYLE, inspectorSections, newElementStyle, patchElementStyle } from '@/lib/canvas/style';

const stroke = (pen?: CanvasPen, points = [{ x: 0, y: 0 }, { x: 20, y: 0 }, { x: 40, y: 20 }, { x: 60, y: 20 }]): CanvasElement =>
  ({ id: 'ink', type: 'freedraw', x: 5, y: 5, width: 60, height: 20, stroke: '#123456', fill: 'transparent', strokeWidth: 4, style: 'clean', seed: 2, points, ...(pen ? { pen } : {}) });
const document = (elements: CanvasElement[]): CanvasDocument => ({ type: 'oculus-canvas', version: 1, elements, viewport: { x: 0, y: 0, zoom: 1 }, background: '#ffffff' });
/** The discs of an outline as [centre x, centre y, radius]. */
const discs = (d: string) => [...d.matchAll(/M(-?[\d.]+) (-?[\d.]+)a([\d.]+)/g)].map(match => [Number(match[1]) + Number(match[3]), Number(match[2]), Number(match[3])]);

describe('pens', () => {
  test('every pen and marker is a saved choice, and an unknown one is refused', () => {
    for (const pen of [...PENS, ...MARKERS]) expect(parseCanvasDocument(serializeCanvasDocument(document([stroke(pen)]))).elements[0].pen).toBe(pen);
    expect(() => serializeCanvasDocument(document([stroke('quill' as CanvasPen)]))).toThrow('invalid pen');
  });

  test('a stroke without a pen is a ball pen: one solid round line, with the SVG it always had', () => {
    expect(penLine(stroke())).toEqual({ dash: [], cap: 'round', join: 'round', passes: [{ dx: 0, dy: 0, width: 4, alpha: 1 }] });
    expect(exportCanvasSvg(document([stroke()]))).toContain('<path d="M 0 0 Q 20 0 30 10 Q 40 20 50 20 L 60 20" fill="none" stroke="#123456" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>');
  });

  test('line pens differ by dash, ends and passes', () => {
    expect(penLine(stroke('dashed'))!.dash).toEqual([16, 12]);
    expect(penLine(stroke('dotted'))!.dash).toEqual([0, 10]);
    expect(penLine(stroke('flatMarker'))).toMatchObject({ cap: 'butt', join: 'bevel' });
    expect(penLine(stroke('roundMarker'))).toEqual(penLine(stroke()));
    for (const pen of ['pencil', 'sketch'] as const) expect(penLine(stroke(pen))!.passes).toHaveLength(2);
    expect(exportCanvasSvg(document([stroke('dotted')]))).toContain('stroke-dasharray="0 10"');
    expect(exportCanvasSvg(document([stroke('flatMarker')]))).toContain('stroke-linecap="butt"');
  });

  test('only a single solid round line is built piece by piece while it is drawn', () => {
    expect([...PENS, ...MARKERS].filter(pen => paintsIncrementally(stroke(pen)))).toEqual(['ball', 'roundMarker']);
  });

  test('samples follow the smoothed curve from the first point to the last', () => {
    const samples = strokeSamples(stroke().points!, 1);
    expect(samples[0]).toEqual({ x: 0, y: 0 });
    expect(samples.at(-1)).toEqual({ x: 60, y: 20 });
    expect(samples.some(point => Math.abs(point.x - 30) < 0.01 && Math.abs(point.y - 10) < 0.01)).toBe(true);
    for (let i = 1; i < samples.length; i++) expect(Math.hypot(samples[i].x - samples[i - 1].x, samples[i].y - samples[i - 1].y)).toBeLessThan(1.6);
  });

  test('a fountain pen is broad across its nib and a hairline along it', () => {
    const across = discs(nibOutline(stroke('fountain', [{ x: 0, y: 0 }, { x: 30, y: -30 }])));
    const along = discs(nibOutline(stroke('fountain', [{ x: 0, y: 0 }, { x: 30, y: 30 }])));
    expect(across[5][2]).toBeCloseTo(2.4, 1);
    expect(along[5][2]).toBeCloseTo(0.6, 1);
  });

  test('a brush swells from a thin start and both brushes lift to a point', () => {
    const line = [{ x: 0, y: 0 }, { x: 200, y: 0 }];
    const brush = discs(nibOutline(stroke('brush', line))), chinese = discs(nibOutline(stroke('chinese', line)));
    const middle = brush[Math.floor(brush.length / 2)][2];
    expect(brush[0][2]).toBeLessThan(middle * 0.4);
    expect(brush.at(-1)![2]).toBeLessThan(middle * 0.2);
    expect(chinese[2][2]).toBeGreaterThan(chinese[Math.floor(chinese.length * 0.7)][2]);
    expect(chinese.at(-1)![2]).toBeLessThan(0.3);
  });

  test('a nib pen exports its outline filled, and a tap is one dot', () => {
    expect(exportCanvasSvg(document([stroke('brush')]))).toMatch(/<path d="M[^"]+Z" fill="#123456" fill-rule="nonzero"\/>/);
    expect(discs(nibOutline(stroke('fountain', [{ x: 0, y: 0 }, { x: 0, y: 0 }])))).toEqual([[0, 0, 2]]);
    expect(nibOutline(stroke('dashed'))).toBe('');
  });

  test('the pen is a style of freehand strokes only', () => {
    expect(inspectorSections([{ type: 'freedraw', label: false }])).toEqual(['stroke', 'pen', 'strokeWidth', 'opacity']);
    expect(inspectorSections([{ type: 'rectangle', label: false }])).not.toContain('pen');
    expect(newElementStyle('freedraw', DEFAULT_DRAWING_STYLE)).not.toHaveProperty('pen');
    expect(newElementStyle('freedraw', { ...DEFAULT_DRAWING_STYLE, pen: 'brush' }).pen).toBe('brush');
    const box = { ...stroke(), type: 'rectangle' as const };
    expect(patchElementStyle(box, { pen: 'brush' })).toBe(box);
    expect(patchElementStyle(stroke(), { pen: 'brush' }).pen).toBe('brush');
  });
});
