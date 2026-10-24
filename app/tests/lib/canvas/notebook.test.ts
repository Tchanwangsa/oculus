import { describe, expect, test } from 'bun:test';
import { parseCanvasDocument, serializeCanvasDocument } from '../../../src/lib/canvas/model';
import { blankNotebook, NOTEBOOK_DESK, notebookFromPages } from '../../../src/lib/canvas/notebook';
import { pageRects, selectableElements } from '../../../src/lib/canvas/paper';

const PNG = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';
const image = (name: string) => ({ dataUrl: PNG, width: 1200, height: 1600, name });

describe('blankNotebook', () => {
  test('is one A4 page of the chosen template on a grey desk', () => {
    const doc = blankNotebook('ruled');
    expect(doc.background).toBe(NOTEBOOK_DESK);
    expect(doc.elements).toEqual([]);
    expect(doc.paper!.pages).toEqual([{ id: 'p1', width: 595, height: 842, template: 'ruled' }]);
  });
  test('landscape turns the page on its side', () => {
    expect(blankNotebook('grid', 'landscape').paper!.pages[0]).toMatchObject({ width: 842, height: 595 });
  });
  test('serialises as a valid canvas', () => {
    expect(() => parseCanvasDocument(serializeCanvasDocument(blankNotebook('dotted')))).not.toThrow();
  });
});

describe('notebookFromPages', () => {
  const sizes = [{ width: 595.28, height: 841.89 }, { width: 612, height: 792 }];
  const doc = notebookFromPages(sizes, [image('a — page 1'), image('a — page 2')]);

  test('has a page for every PDF page, at the PDF page size', () => {
    expect(doc.paper!.pages.map(p => [p.width, p.height, p.template])).toEqual([[595.28, 841.89, 'blank'], [612, 792, 'blank']]);
  });
  test('lays each picture exactly on its page, locked', () => {
    const rects = pageRects(doc.paper!);
    doc.elements.forEach((element, i) => {
      expect(element).toMatchObject({ type: 'image', x: rects[i].x, y: rects[i].y, width: rects[i].width, height: rects[i].height, locked: true, naturalWidth: 1200, naturalHeight: 1600 });
    });
    expect(doc.elements.map(e => e.sourceName)).toEqual(['a — page 1', 'a — page 2']);
  });
  test('the pictures are backdrops: nothing in the notebook is pickable yet', () => {
    expect(selectableElements(doc.elements)).toEqual([]);
  });
  test('the pictures sit in separate pages, so each belongs to its own', () => {
    const rects = pageRects(doc.paper!);
    expect(doc.elements[1].y).toBe(rects[1].y);
    expect(doc.elements[1].y).toBeGreaterThan(doc.elements[0].y + doc.elements[0].height);
  });
  test('serialises as a valid canvas and round-trips', () => {
    const again = parseCanvasDocument(serializeCanvasDocument(doc));
    expect(again.paper).toEqual(doc.paper);
    expect(again.elements).toHaveLength(2);
  });
  test('needs one picture per page', () => {
    expect(() => notebookFromPages(sizes, [image('only')])).toThrow();
    expect(() => notebookFromPages([], [])).toThrow();
  });
});
