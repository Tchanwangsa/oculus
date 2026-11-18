import { describe, expect, test } from 'bun:test';
import { applyElementChanges } from '../../../src/lib/canvas/changes';
import { commitShapeLabel, growToFitLabel, labelTargetAt, restyleElement, stylePatchChanges } from '../../../src/lib/canvas/labels';
import { labelBox, layoutShapeLabel, measureTextElement, type TextMeasure } from '../../../src/lib/canvas/renderText';
import type { CanvasElement } from '../../../src/lib/canvas/model';

const measure: TextMeasure = (line, size) => line.length * size * 0.5;
const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const rect: CanvasElement = { id: 'r', type: 'rectangle', x: 0, y: 0, width: 200, height: 60, ...base, fontSize: 20 };
const text: CanvasElement = { id: 't', type: 'text', x: 5, y: 5, width: 50, height: 28, ...base, text: 'abcde', fontSize: 20 };
const arrow: CanvasElement = { id: 'a', type: 'arrow', x: 0, y: 0, width: 50, height: 0, ...base, points: [{ x: 0, y: 0 }, { x: 50, y: 0 }] };

describe('element changes', () => {
  test('replace in place, remove and append while keeping untouched identities', () => {
    const elements = [rect, text, arrow];
    const moved = { ...text, x: 9 }, added = { ...rect, id: 'n' };
    const next = applyElementChanges(elements, new Map<string, CanvasElement | null>([['t', moved], ['a', null], ['n', added]]));
    expect(next.map(el => el.id)).toEqual(['r', 't', 'n']);
    expect(next[0]).toBe(rect); expect(next[1]).toBe(moved);
  });
  test('returns the same array when nothing changes', () => {
    const elements = [rect, text];
    expect(applyElementChanges(elements, new Map())).toBe(elements);
    expect(applyElementChanges(elements, new Map([['r', rect], ['missing', null]]))).toBe(elements);
  });
});

describe('shape labels', () => {
  test('commit stores the text and an empty commit removes only the label', () => {
    const labelled = commitShapeLabel(rect, 'Hello', measure);
    expect(labelled).toMatchObject({ id: 'r', type: 'rectangle', text: 'Hello', width: 200, height: 60 });
    const cleared = commitShapeLabel(labelled, '  \n ', measure);
    expect(cleared).not.toHaveProperty('text');
    expect(cleared).toMatchObject({ id: 'r', width: 200, height: 60, fontSize: 20 });
    expect(commitShapeLabel(rect, '', measure)).toBe(rect);
  });
  test('a label taller than its box grows the shape downward and never shrinks it', () => {
    const long = commitShapeLabel(rect, 'one\ntwo\nthree', measure);
    expect(long.height).toBeCloseTo(3 * 20 * 1.4 + 16);
    expect(long.y).toBe(0);
    expect(layoutShapeLabel(long, measure).height).toBeLessThanOrEqual(labelBox(long).height + 1e-6);
    const tall = { ...rect, height: 400, text: 'short' };
    expect(growToFitLabel(tall, measure)).toBe(tall);
    expect(growToFitLabel(rect, measure)).toBe(rect);
  });
  test('growing keeps the top edge of a shape drawn upward and uses the inscribed box of ellipses and diamonds', () => {
    const upward = growToFitLabel({ ...rect, y: 100, height: -60, text: 'a\nb\nc' }, measure);
    expect(upward.y).toBe(40);
    expect(upward.height).toBeGreaterThan(60);
    const diamond = growToFitLabel({ ...rect, type: 'diamond', width: 400, text: 'a\nb' }, measure);
    expect(diamond.height).toBeCloseTo((2 * 28 + 16) / 0.5);
    const ellipse = growToFitLabel({ ...rect, type: 'ellipse', width: 400, text: 'a\nb' }, measure);
    expect(ellipse.height).toBeCloseTo((2 * 28 + 16) / Math.SQRT1_2);
  });
  test('wrapping to a narrower box grows the shape', () => {
    const narrow = commitShapeLabel({ ...rect, width: 60 }, 'aaaa bbbb', measure);
    expect(layoutShapeLabel(narrow, measure).lines.length).toBe(2);
    expect(narrow.height).toBeCloseTo(2 * 28 + 16);
  });
  test('finds the topmost shape under a point, including inside hollow shapes', () => {
    const ellipse: CanvasElement = { ...rect, id: 'e', type: 'ellipse', x: 0, y: 0, width: 100, height: 100 };
    expect(labelTargetAt([rect, text], { x: 100, y: 30 })?.id).toBe('r');
    expect(labelTargetAt([rect, { ...rect, id: 'top' }], { x: 100, y: 30 })?.id).toBe('top');
    expect(labelTargetAt([ellipse], { x: 50, y: 50 })?.id).toBe('e');
    expect(labelTargetAt([ellipse], { x: 2, y: 2 }, 0)).toBeUndefined();
    expect(labelTargetAt([text, arrow], { x: 10, y: 10 })).toBeUndefined();
  });
});

describe('style patches', () => {
  test('write only the fields that apply to each type', () => {
    const line: CanvasElement = { ...arrow, id: 'l', type: 'line' };
    const changes = stylePatchChanges([rect, text, arrow, line], new Set(['r', 't', 'a', 'l']), { fill: '#e6e5fb', strokeWidth: 4, endArrowhead: 'dot', verticalAlign: 'top' }, measure);
    expect(changes.get('r')).toMatchObject({ fill: '#e6e5fb', strokeWidth: 4, verticalAlign: 'top' });
    expect(changes.get('r')).not.toHaveProperty('endArrowhead');
    expect(changes.has('t')).toBe(false);
    expect(changes.get('a')).toMatchObject({ fill: 'transparent', strokeWidth: 4, endArrowhead: 'dot' });
    expect(changes.get('a')).not.toHaveProperty('verticalAlign');
    expect(changes.get('l')).not.toHaveProperty('endArrowhead');
  });
  test('only selected elements that actually change are returned', () => {
    expect(stylePatchChanges([rect, text], new Set(['t']), { fill: '#e6e5fb' }, measure).size).toBe(0);
    expect(stylePatchChanges([rect], new Set(['r']), { opacity: 1, strokeStyle: 'solid' }, measure).size).toBe(0);
    expect(stylePatchChanges([rect], new Set(['r']), { stroke: '#111111' }, measure).size).toBe(0);
  });
  test('a font change re-measures standalone text', () => {
    const bigger = restyleElement(text, { fontSize: 40 }, measure);
    expect(bigger).toMatchObject({ fontSize: 40, width: 100, height: 56 });
    expect(restyleElement(text, { textAlign: 'center' }, measure)).toMatchObject({ width: 50, height: 28, textAlign: 'center' });
    expect(measureTextElement({ ...text, text: 'ab\nabcd' }, measure)).toMatchObject({ width: 40, height: 56 });
  });
  test('a font change on a labelled shape re-runs the grow rule', () => {
    const labelled = commitShapeLabel(rect, 'Hi', measure);
    expect(labelled.height).toBe(60);
    const grown = restyleElement(labelled, { fontSize: 48 }, measure);
    expect(grown.height).toBeCloseTo(48 * 1.4 + 16);
    expect(restyleElement(grown, { fontSize: 12 }, measure).height).toBe(grown.height);
  });
});
