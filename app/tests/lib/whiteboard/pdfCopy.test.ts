import { describe, expect, test } from 'bun:test';
import { parseCanvasDocument, serializeCanvasDocument } from '../../../src/lib/canvas/model';
import { boardFromPages } from '../../../src/lib/canvas/pdfCopy';
import { selectableElements } from '../../../src/lib/canvas/paper';

const PNG = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';
const image = (name: string) => ({ dataUrl: PNG, width: 1200, height: 1600, name });

describe('boardFromPages', () => {
  const sizes = [{ width: 595.28, height: 841.89 }, { width: 612, height: 792 }, { width: 842, height: 595 }];
  const doc = boardFromPages(sizes, [image('a — page 1'), image('a — page 2'), image('a — page 3')]);

  test('is a canvas: no paper, the default white board', () => {
    expect(doc.paper).toBeUndefined();
    expect(doc.background).toBe('#ffffff');
  });
  test('each page is an ordinary image at its PDF size, nothing locked', () => {
    expect(doc.elements.map(e => [e.type, e.width, e.height, e.locked ?? false])).toEqual([
      ['image', 595.28, 841.89, false], ['image', 612, 792, false], ['image', 842, 595, false],
    ]);
    expect(selectableElements(doc.elements)).toHaveLength(3);
    expect(doc.elements.map(e => e.sourceName)).toEqual(['a — page 1', 'a — page 2', 'a — page 3']);
  });
  test('the pages stack top to bottom in page order with a gap, left edges aligned', () => {
    const [a, b, c] = doc.elements;
    expect(new Set(doc.elements.map(e => e.x)).size).toBe(1);
    expect(b.y - (a.y + a.height)).toBeGreaterThan(0);
    expect(c.y - (b.y + b.height)).toBe(b.y - (a.y + a.height));
  });
  test('the first page is in view at the default camera', () => {
    expect(doc.elements[0].x).toBeGreaterThanOrEqual(0);
    expect(doc.elements[0].y).toBeGreaterThanOrEqual(0);
  });
  test('serialises as a valid canvas and round-trips', () => {
    const again = parseCanvasDocument(serializeCanvasDocument(doc));
    expect(again.paper).toBeUndefined();
    expect(again.elements).toHaveLength(3);
  });
  test('needs one picture per page', () => {
    expect(() => boardFromPages(sizes, [image('only')])).toThrow();
    expect(() => boardFromPages([], [])).toThrow();
  });
});
