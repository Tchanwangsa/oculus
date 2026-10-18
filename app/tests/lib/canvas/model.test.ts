import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, parseCanvasDocument, roundnessOf, serializeCanvasDocument, type CanvasElement } from '../../../src/lib/canvas/model';

const element: CanvasElement = { id: 'shape', type: 'rectangle', x: -10, y: 4, width: 100, height: -40, stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'sketch', seed: 9 };

describe('canvas document format', () => {
  test('new documents do not share mutable data', () => {
    const a = createCanvasDocument(), b = createCanvasDocument();
    a.elements.push(element); a.viewport.x = 5;
    expect(b.elements).toEqual([]); expect(b.viewport.x).toBe(0);
  });
  test('roundtrips extension fields at every level without mutation', () => {
    const doc = { ...createCanvasDocument(), plugins: { mindmap: { version: 2 } }, viewport: { x: 2, y: 4, zoom: 0.7, grid: false }, elements: [{ ...element, linkedNote: 'note.md', custom: { nested: [1, 'two'] } }] };
    const saved = serializeCanvasDocument(doc);
    expect(parseCanvasDocument(saved)).toEqual(doc);
    expect(doc.elements[0].height).toBe(-40);
  });
  test('refuses unknown versions and element types instead of destroying data', () => {
    expect(() => parseCanvasDocument(JSON.stringify({ ...createCanvasDocument(), version: 2 }))).toThrow('Unsupported canvas version');
    expect(() => parseCanvasDocument(JSON.stringify({ ...createCanvasDocument(), elements: [{ ...element, type: 'plugin-widget' }] }))).toThrow('Unsupported canvas element type');
  });
  test.each([
    { viewport: { x: 0, y: 0, zoom: 0 } },
    { elements: [{ ...element, points: [{ x: '2', y: 0 }] }] },
    { elements: [{ ...element, strokeWidth: -1 }] },
    { elements: [element, element] },
    { elements: [{ ...element, fontSize: 0 }] },
    { elements: [{ ...element, text: {} }] },
  ])('refuses malformed geometry: %j', patch => {
    expect(() => parseCanvasDocument(JSON.stringify({ ...createCanvasDocument(), ...patch }))).toThrow();
  });
  test('accepts every optional appearance, arrowhead and binding field', () => {
    const arrow = { ...element, id: 'arrow', type: 'arrow' as const, startArrowhead: 'dot' as const, endArrowhead: 'none' as const, strokeStyle: 'dotted' as const, opacity: 0,
      startBinding: { elementId: 'shape', mode: 'center' as const }, endBinding: { elementId: 'gone', mode: 'edge' as const, anchor: { x: 0, y: 1 } } };
    const labelled = { ...element, text: 'Idea', fontSize: 18, fontFamily: 'serif' as const, textAlign: 'right' as const, verticalAlign: 'bottom' as const, strokeStyle: 'dashed' as const, opacity: 0.5, roundness: 'sharp' as const };
    const doc = { ...createCanvasDocument(), elements: [labelled, arrow, { ...element, id: 'text', type: 'text' as const, text: 'x', fontFamily: 'mono' as const, textAlign: 'center' as const, opacity: 1 }] };
    expect(parseCanvasDocument(serializeCanvasDocument(doc))).toEqual(doc);
  });
  test.each([
    ['opacity', 1.5], ['opacity', -0.1], ['opacity', '1'], ['strokeStyle', 'wavy'], ['fontFamily', 'cursive'], ['textAlign', 'justify'],
    ['verticalAlign', 'center'], ['startArrowhead', 'circle'], ['endArrowhead', null], ['roundness', 'rounded'], ['roundness', 8],
    ['startBinding', { elementId: '', mode: 'center' }], ['startBinding', { elementId: 'a', mode: 'corner' }], ['endBinding', 'shape'],
    ['endBinding', { elementId: 'a', mode: 'edge' }], ['endBinding', { elementId: 'a', mode: 'edge', anchor: { x: 1.2, y: 0 } }],
  ])('refuses an invalid %s: %j', (key, value) => {
    expect(() => parseCanvasDocument(JSON.stringify({ ...createCanvasDocument(), elements: [{ ...element, [key]: value }] }))).toThrow(`invalid ${key}`);
  });
  test('reports malformed JSON and refuses non-finite geometry on save', () => {
    expect(() => parseCanvasDocument('{')).toThrow('invalid JSON');
    expect(() => serializeCanvasDocument({ ...createCanvasDocument(), elements: [{ ...element, x: Infinity }] })).toThrow('invalid x');
  });
});

describe('roundness', () => {
  test('is round for rectangles and sharp for diamonds when absent', () => {
    expect(roundnessOf({ ...element, type: 'rectangle' })).toBe('round');
    expect(roundnessOf({ ...element, type: 'diamond' })).toBe('sharp');
    expect(roundnessOf({ ...element, type: 'diamond', roundness: 'round' })).toBe('round');
  });
});
