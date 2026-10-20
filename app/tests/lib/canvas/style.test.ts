import { describe, expect, test } from 'bun:test';
import {
  adoptElementStyle, colorLabel, DEFAULT_DRAWING_STYLE, inspectorSections, inspectorTarget, inspectorValues, newElementStyle, patchDrawingStyle,
  patchElementStyle,
  type InspectorTarget,
} from '../../../src/lib/canvas/style';
import type { CanvasElement } from '../../../src/lib/canvas/model';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const rect: CanvasElement = { id: 'r', type: 'rectangle', x: 0, y: 0, width: 200, height: 60, ...base };
const text: CanvasElement = { id: 't', type: 'text', x: 0, y: 0, width: 50, height: 28, ...base, text: 'abc', fontSize: 20, fontFamily: 'serif' };
const tool = (type: CanvasElement['type']): InspectorTarget => ({ type, label: false });

describe('inspector sections', () => {
  test('follow the element type', () => {
    expect(inspectorSections([tool('text')])).toEqual(['stroke', 'text', 'opacity']);
    expect(inspectorSections([tool('rectangle')])).toEqual(['stroke', 'fill', 'strokeWidth', 'strokeStyle', 'sketch', 'roundness', 'opacity']);
    expect(inspectorSections([tool('diamond')])).toContain('roundness');
    expect(inspectorSections([tool('ellipse')])).not.toContain('roundness');
    expect(inspectorSections([tool('arrow')])).toEqual(['stroke', 'strokeWidth', 'strokeStyle', 'sketch', 'arrowType', 'arrowheads', 'opacity']);
    expect(inspectorSections([tool('line')])).toEqual(['stroke', 'strokeWidth', 'strokeStyle', 'sketch', 'opacity']);
    expect(inspectorSections([tool('freedraw')])).toEqual(['stroke', 'strokeWidth', 'opacity']);
    expect(inspectorSections([])).toEqual([]);
  });
  test('text options show on a shape once it has a label or one is being written', () => {
    expect(inspectorSections([inspectorTarget({ ...rect, text: 'Label' })])).toEqual(['stroke', 'fill', 'text', 'verticalAlign', 'strokeWidth', 'strokeStyle', 'sketch', 'roundness', 'opacity']);
    expect(inspectorSections([inspectorTarget({ ...rect, text: '   ' })])).not.toContain('text');
    expect(inspectorSections([inspectorTarget(rect, true)])).toContain('verticalAlign');
  });
  test('a mixed selection shows the union', () => {
    expect(inspectorSections([inspectorTarget(text), inspectorTarget({ ...rect, type: 'arrow' })])).toEqual(['stroke', 'text', 'strokeWidth', 'strokeStyle', 'sketch', 'arrowType', 'arrowheads', 'opacity']);
  });
  test('the colour is called Color only when every target is text', () => {
    expect(colorLabel([tool('text')])).toBe('Color');
    expect(colorLabel([tool('text'), tool('rectangle')])).toBe('Stroke');
  });
});

describe('inspector values', () => {
  test('each field reads from the first target that shows it, with type defaults for absent fields', () => {
    const values = inspectorValues([inspectorTarget({ ...rect, stroke: '#5e6ad2' }), inspectorTarget(text)], DEFAULT_DRAWING_STYLE);
    expect(values).toMatchObject({ stroke: '#5e6ad2', fontFamily: 'serif', fontSize: 20, textAlign: 'left', opacity: 1, strokeStyle: 'solid' });
    expect(inspectorValues([inspectorTarget({ ...rect, text: 'x' })], DEFAULT_DRAWING_STYLE)).toMatchObject({ textAlign: 'center', verticalAlign: 'middle', fontSize: 24 });
  });
  test('with nothing selected the tool reads the drawing defaults, labels using their own alignment', () => {
    const style = { ...DEFAULT_DRAWING_STYLE, textAlign: 'right' as const, labelAlign: 'left' as const };
    expect(inspectorValues([tool('text')], style).textAlign).toBe('right');
    expect(inspectorValues([tool('rectangle')], style).textAlign).toBe('left');
  });
});

describe('drawing defaults', () => {
  test('new elements get base fields and only applicable non-default options', () => {
    const style = { ...DEFAULT_DRAWING_STYLE, fill: '#e6e5fb', fontFamily: 'mono' as const, endArrowhead: 'dot' as const, opacity: 0.5 };
    expect(newElementStyle('rectangle', DEFAULT_DRAWING_STYLE)).toEqual({ stroke: '#292b32', fill: 'transparent', strokeWidth: 2, style: 'sketch' });
    expect(newElementStyle('rectangle', style)).toEqual({ stroke: '#292b32', fill: '#e6e5fb', strokeWidth: 2, style: 'sketch', opacity: 0.5, fontFamily: 'mono' });
    expect(newElementStyle('arrow', style)).toEqual({ stroke: '#292b32', fill: 'transparent', strokeWidth: 2, style: 'sketch', opacity: 0.5, endArrowhead: 'dot' });
    expect(newElementStyle('text', { ...style, textAlign: 'center' })).toMatchObject({ fill: 'transparent', fontFamily: 'mono', textAlign: 'center' });
    expect(newElementStyle('ellipse', { ...style, labelAlign: 'right' })).toMatchObject({ textAlign: 'right' });
    expect(newElementStyle('ellipse', { ...style, textAlign: 'right' })).not.toHaveProperty('textAlign');
  });
  test('alignment patches update the default for the kinds they were made on', () => {
    expect(patchDrawingStyle(DEFAULT_DRAWING_STYLE, { textAlign: 'right' }, ['text'])).toMatchObject({ textAlign: 'right', labelAlign: 'center' });
    expect(patchDrawingStyle(DEFAULT_DRAWING_STYLE, { textAlign: 'right' }, ['diamond'])).toMatchObject({ textAlign: 'left', labelAlign: 'right' });
    expect(patchDrawingStyle(DEFAULT_DRAWING_STYLE, { fontSize: 36, stroke: '#fff' }, [])).toMatchObject({ fontSize: 36, stroke: '#fff' });
  });
  test('selecting an element adopts the fields that apply to it', () => {
    const adopted = adoptElementStyle({ ...DEFAULT_DRAWING_STYLE, fill: '#e6e5fb' }, { ...text, textAlign: 'center' });
    expect(adopted).toMatchObject({ stroke: '#111111', fontFamily: 'serif', fontSize: 20, textAlign: 'center', labelAlign: 'center', fill: '#e6e5fb', strokeWidth: 2 });
    expect(adoptElementStyle(DEFAULT_DRAWING_STYLE, { ...rect, textAlign: 'right' })).toMatchObject({ labelAlign: 'right', textAlign: 'left', fill: 'transparent' });
  });
});

describe('roundness', () => {
  const diamond: CanvasElement = { ...rect, id: 'd', type: 'diamond' };
  test('applies to rectangles and diamonds only, and a patch that changes nothing returns the element', () => {
    expect(patchElementStyle(rect, { roundness: 'sharp' })).toMatchObject({ roundness: 'sharp' });
    expect(patchElementStyle(rect, { roundness: 'round' })).toBe(rect);
    expect(patchElementStyle(diamond, { roundness: 'sharp' })).toBe(diamond);
    expect(patchElementStyle(diamond, { roundness: 'round' })).toMatchObject({ roundness: 'round' });
    const ellipse: CanvasElement = { ...rect, type: 'ellipse' };
    expect(patchElementStyle(ellipse, { roundness: 'sharp' })).toBe(ellipse);
  });
  test('reads each type default and is written only where it differs', () => {
    expect(inspectorValues([inspectorTarget(rect)], DEFAULT_DRAWING_STYLE).roundness).toBe('round');
    expect(inspectorValues([inspectorTarget(diamond)], DEFAULT_DRAWING_STYLE).roundness).toBe('sharp');
    expect(newElementStyle('rectangle', DEFAULT_DRAWING_STYLE)).not.toHaveProperty('roundness');
    expect(newElementStyle('diamond', DEFAULT_DRAWING_STYLE)).toMatchObject({ roundness: 'round' });
    expect(newElementStyle('ellipse', { ...DEFAULT_DRAWING_STYLE, roundness: 'sharp' })).not.toHaveProperty('roundness');
    expect(adoptElementStyle(DEFAULT_DRAWING_STYLE, diamond).roundness).toBe('sharp');
  });
});
