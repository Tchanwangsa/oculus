import { describe, expect, test } from 'bun:test';
import { applyElementChanges } from '../../../src/lib/canvas/changes';
import { commitTextEdit } from '../../../src/lib/canvas/labels';
import { exportCanvasSvg } from '../../../src/lib/canvas/renderSvg';
import type { CanvasDocument, CanvasElement } from '../../../src/lib/canvas/model';

const appearance = { stroke: '#292b32', fill: 'transparent', strokeWidth: 2, style: 'sketch' as const };
const shape: CanvasElement = { id: 'shape', type: 'rectangle', x: 10, y: 20, width: 90, height: 60, ...appearance, seed: 9 };
const label: CanvasElement = { ...shape, id: 'label', type: 'text', text: 'Original label', fontSize: 24, futureMetadata: { link: 'lecture-1' } };

function document(elements: CanvasElement[]): CanvasDocument {
  return { type: 'oculus-canvas', version: 1, elements, viewport: { x: 0, y: 0, zoom: 1 }, background: '#ffffff' };
}

describe('canvas text edits', () => {
  const measure = (line: string, size: number) => line.length * size * 0.5;
  test('editing existing text re-measures it in place without changing its layer or extension data', () => {
    const front = { ...shape, id: 'front' };
    const before = [shape, label, front];
    const edited = commitTextEdit({ ...label, fontSize: 20 }, 'text', 'New label\nsecond line', measure);
    const after = applyElementChanges(before, new Map([[label.id, edited]]));
    expect(after.map(element => element.id)).toEqual(['shape', 'label', 'front']);
    expect(after[1]).toMatchObject({ text: 'New label\nsecond line', x: 10, y: 20, width: 110, height: 56, fontSize: 20, seed: 9, futureMetadata: { link: 'lecture-1' } });
    expect(after[0]).toBe(shape);
    expect(after[2]).toBe(front);
    expect(before[1].text).toBe('Original label');
    expect(label.width).toBe(90);
  });

  test('a blank edit deletes only that text, and blank new text adds nothing', () => {
    expect(commitTextEdit(label, 'text', ' \n ', measure)).toBeNull();
    expect(applyElementChanges([shape, label], new Map([[label.id, null]]))).toEqual([shape]);
    expect(applyElementChanges([shape], new Map([['new', null]]))).toEqual([shape]);
    const added = commitTextEdit({ ...label, id: 'new' }, 'text', 'Added label', measure)!;
    expect(applyElementChanges([shape], new Map([['new', added]]))).toEqual([shape, added]);
  });

  test('a blank label edit keeps the shape and drops only its text', () => {
    const labelled = commitTextEdit(shape, 'label', 'Label', measure)!;
    expect(labelled).toMatchObject({ id: 'shape', type: 'rectangle', text: 'Label' });
    expect(commitTextEdit(labelled, 'label', '', measure)).toEqual(shape);
  });
});

describe('canvas vector export', () => {
  test('exports the entire drawing independently of the current pan and zoom', () => {
    const content = document([shape, { ...label, x: -140, text: '<concept> & "connections"' }]);
    const exported = exportCanvasSvg(content);
    expect(exportCanvasSvg({ ...content, viewport: { x: 9999, y: -8000, zoom: 8 } })).toBe(exported);
    expect(exported).toContain('&lt;concept&gt; &amp; &quot;connections&quot;');
    expect(exported).not.toContain('<concept>');
    expect(exported).toContain('viewBox="-172 -12 304 124"');
  });

  test('supports all tools, reversed dimensions, and attribute escaping with finite geometry', () => {
    const types = ['rectangle', 'ellipse', 'diamond', 'line', 'arrow', 'freedraw', 'text'] as const;
    const elements = types.map((type, i) => ({ ...shape, id: String(i), type, x: i * 100, width: -80, height: -40, points: [{ x: 0, y: 0 }, { x: -30, y: -50 }, { x: -80, y: -40 }], text: 'A\nB' }));
    const exported = exportCanvasSvg({ ...document(elements), background: '#fff" onload="bad' });
    expect(exported).toContain('<ellipse');
    expect(exported).toContain('<polygon');
    expect(exported).toContain('<path');
    expect(exported).toMatch(/<tspan x="-80" y="[0-9.-]+">B<\/tspan>/);
    expect(exported).not.toMatch(/(?:NaN|Infinity|width="-80")/);
    expect(exported).not.toContain('fill="#fff" onload="bad"');
    expect(exported).toContain('fill="#fff&quot; onload=&quot;bad"');
  });

  test('writes opacity, dashes, fonts and label alignment', () => {
    const measure = (line: string) => line.length * 10;
    const labelled = { ...shape, width: 160, height: 100, text: 'one two three four', fontSize: 20, fontFamily: 'serif' as const, textAlign: 'right' as const, strokeStyle: 'dashed' as const, opacity: 0.5 };
    const exported = exportCanvasSvg(document([labelled, { ...label, id: 't', fontFamily: 'mono', textAlign: 'center', strokeStyle: 'dotted' }, { ...shape, id: 'f', type: 'freedraw', strokeStyle: 'dashed', points: [{ x: 0, y: 0 }, { x: 5, y: 5 }] }]), measure);
    expect(exported).toContain('opacity="0.5"');
    expect(exported).toContain('stroke-dasharray="8 6"');
    expect(exported.match(/stroke-dasharray/g)).toHaveLength(2);
    expect(exported).toContain('Georgia');
    expect(exported).toContain('ui-monospace');
    expect(exported).toContain('text-anchor="end"');
    expect(exported).toContain('text-anchor="middle"');
    expect(exported).toContain('<tspan x="152" y="26">one two three</tspan><tspan x="152" y="54">four</tspan>');
    expect(exported.match(/<text/g)).toHaveLength(2);
  });

  test('draws each arrowhead type solid and filled where closed', () => {
    const arrow: CanvasElement = { ...shape, id: 'a', type: 'arrow', style: 'clean', strokeStyle: 'dashed', points: [{ x: 0, y: 0 }, { x: 100, y: 0 }] };
    const svg = (patch: Partial<CanvasElement>) => exportCanvasSvg(document([{ ...arrow, ...patch }]));
    const heads = (text: string) => text.match(/<(?:path|circle)[^>]*>/g)!.slice(1);
    expect(heads(svg({}))).toEqual([expect.stringMatching(/^<path d="M [^Z]+" fill="none"/)]);
    expect(heads(svg({ startArrowhead: 'triangle', endArrowhead: 'diamond' }))).toEqual([
      expect.stringMatching(/^<path d="M .+ Z" fill="#292b32"/), expect.stringMatching(/^<path d="M .+ Z" fill="#292b32"/),
    ]);
    expect(heads(svg({ endArrowhead: 'dot' }))).toEqual([expect.stringMatching(/^<circle cx="93.6" cy="0" r="6.4" fill="#292b32"/)]);
    expect(heads(svg({ endArrowhead: 'bar' }))).toEqual(['<path d="M 100 -8 L 100 8" fill="none" stroke="#292b32" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>']);
    expect(heads(svg({ endArrowhead: 'none' }))).toEqual([]);
    for (const head of heads(svg({ startArrowhead: 'arrow', endArrowhead: 'triangle' }))) expect(head).not.toContain('dasharray');
  });
});
