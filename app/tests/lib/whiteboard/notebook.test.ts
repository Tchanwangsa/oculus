import { beforeEach, describe, expect, mock, test } from 'bun:test';
import { parseWhiteboardDocument, serializeWhiteboardDocument } from '../../../src/lib/whiteboard/model';
import { blankNotebook, createBlankNotebook, NOTEBOOK_DESK, notebookFromPages } from '../../../src/lib/whiteboard/notebook';
import { createWhiteboard } from '../../../src/lib/whiteboard/files';
import { pageRects, selectableElements } from '../../../src/lib/whiteboard/paper';

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
  test('serialises as a valid whiteboard', () => {
    expect(() => parseWhiteboardDocument(serializeWhiteboardDocument(blankNotebook('dotted')))).not.toThrow();
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
  test('serialises as a valid whiteboard and round-trips', () => {
    const again = parseWhiteboardDocument(serializeWhiteboardDocument(doc));
    expect(again.paper).toEqual(doc.paper);
    expect(again.elements).toHaveLength(2);
  });
  test('needs one picture per page', () => {
    expect(() => notebookFromPages(sizes, [image('only')])).toThrow();
    expect(() => notebookFromPages([], [])).toThrow();
  });
});

describe('creating', () => {
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  const stop = new Error('stopped after the native call');
  /** Records the native command and refuses it, so nothing reaches the database. */
  beforeEach(() => {
    calls.length = 0;
    mock.module('@tauri-apps/api/core', () => ({
      invoke: async (command: string, args: Record<string, unknown>) => { calls.push({ command, args }); throw stop; },
    }));
  });

  test('a new notebook is a .notebook with pages', async () => {
    await expect(createBlankNotebook({ id: 1, code: 'X' }, 'Week 3', 'grid', 'portrait')).rejects.toBe(stop);
    expect(calls).toHaveLength(1);
    expect(calls[0]).toMatchObject({ command: 'create_whiteboard', args: { subjectCode: 'X', title: 'Week 3', kind: 'notebook' } });
    expect(parseWhiteboardDocument(calls[0].args.content as string).paper?.pages).toHaveLength(1);
  });

  test('a new whiteboard is a .whiteboard without pages', async () => {
    await expect(createWhiteboard({ id: 1, code: 'X' })).rejects.toBe(stop);
    expect(calls[0]).toMatchObject({ command: 'create_whiteboard', args: { kind: 'whiteboard' } });
    expect(parseWhiteboardDocument(calls[0].args.content as string).paper).toBeUndefined();
  });
});
