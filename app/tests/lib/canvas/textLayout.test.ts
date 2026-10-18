import { describe, expect, test } from 'bun:test';
import { canvasFont, labelBox, layoutShapeLabel, layoutTextElement, wrapLines, type TextMeasure } from '../../../src/lib/canvas/renderText';
import type { CanvasElement } from '../../../src/lib/canvas/model';

const tenPerChar = (line: string) => line.length * 10;
const measure: TextMeasure = line => tenPerChar(line);
const shape: CanvasElement = { id: 's', type: 'rectangle', x: 10, y: 20, width: 200, height: 100, stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 1, text: 'one two', fontSize: 20 };

describe('canvas text wrapping', () => {
  test('wraps greedily by words and keeps explicit and empty lines', () => {
    expect(wrapLines('alpha beta gamma', 100, tenPerChar)).toEqual(['alpha beta', 'gamma']);
    expect(wrapLines('a\n\nb', 100, tenPerChar)).toEqual(['a', '', 'b']);
    expect(wrapLines('', 100, tenPerChar)).toEqual(['']);
  });
  test('breaks a word wider than the box between characters', () => {
    expect(wrapLines('abcdefghijkl', 50, tenPerChar)).toEqual(['abcde', 'fghij', 'kl']);
    expect(wrapLines('hi abcdefghijkl', 50, tenPerChar)).toEqual(['hi', 'abcde', 'fghij', 'kl']);
    expect(wrapLines('ab', 0, tenPerChar)).toEqual(['a', 'b']);
  });
});

describe('canvas label layout', () => {
  test('label boxes lose the padding; ellipses and diamonds use their inscribed box', () => {
    expect(labelBox(shape)).toEqual({ x: 8, y: 8, width: 184, height: 84 });
    expect(labelBox({ ...shape, width: -200 })).toEqual({ x: -192, y: 8, width: 184, height: 84 });
    expect(labelBox({ ...shape, type: 'diamond' })).toEqual({ x: 58, y: 33, width: 84, height: 34 });
    const ellipse = labelBox({ ...shape, type: 'ellipse' });
    expect(ellipse.width).toBeCloseTo(200 * Math.SQRT1_2 - 16);
    expect(ellipse.x + ellipse.width / 2).toBeCloseTo(100);
    expect(labelBox({ ...shape, width: 10, height: 4 })).toEqual({ x: 5, y: 2, width: 0, height: 0 });
  });
  test('places lines per horizontal and vertical alignment, centred by default', () => {
    const at = (patch: Partial<CanvasElement>) => layoutShapeLabel({ ...shape, ...patch }, measure).lines.map(({ x, y }) => ({ x, y }));
    expect(at({})).toEqual([{ x: 100, y: 40 }]);
    expect(at({ textAlign: 'left', verticalAlign: 'top' })).toEqual([{ x: 8, y: 12 }]);
    expect(at({ textAlign: 'right', verticalAlign: 'bottom' })).toEqual([{ x: 192, y: 68 }]);
    expect(at({ text: 'one\ntwo' })).toEqual([{ x: 100, y: 26 }, { x: 100, y: 54 }]);
  });
  test('wraps to the label box and measures with the element font', () => {
    const fonts: string[] = [];
    const layout = layoutShapeLabel({ ...shape, type: 'diamond', text: 'aaaa bbbb cccc', fontFamily: 'mono' }, (line, size, family) => { fonts.push(`${size} ${family}`); return line.length * 10; });
    expect(layout.lines.map(line => line.text)).toEqual(['aaaa', 'bbbb', 'cccc']);
    expect(layout).toMatchObject({ fontSize: 20, fontFamily: 'mono', textAlign: 'center', height: 84 });
    expect(new Set(fonts)).toEqual(new Set(['20 mono']));
  });
  test('standalone text aligns its explicit lines within its own width', () => {
    const text: CanvasElement = { ...shape, type: 'text', width: 120, height: 67.2, text: 'ab\ncd', fontSize: 24 };
    expect(layoutTextElement(text).lines.map(line => line.x)).toEqual([0, 0]);
    expect(layoutTextElement({ ...text, textAlign: 'center' }).lines.map(line => line.x)).toEqual([60, 60]);
    expect(layoutTextElement({ ...text, textAlign: 'right' }).lines.map(line => line.x)).toEqual([120, 120]);
    expect(layoutTextElement(text).lines.map(line => line.y)).toEqual([4.8, 38.4].map(y => expect.closeTo(y) as unknown as number));
  });
  test('font strings come from one stack per family', () => {
    expect(canvasFont(16)).toStartWith('16px "Inter Variable"');
    expect(canvasFont(16, 'serif')).toContain('Georgia');
    expect(canvasFont(16, 'mono')).toContain('ui-monospace');
  });
});
