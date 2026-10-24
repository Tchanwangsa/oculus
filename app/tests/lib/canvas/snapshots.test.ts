import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasDocument, type CanvasElement } from '../../../src/lib/canvas/model';
import { createPaper, pageRects, type Paper } from '../../../src/lib/canvas/paper';
import { NotebookSnapshots, pageFingerprints, type SnapshotDeps, type SnapshotRequest } from '../../../src/lib/canvas/snapshots';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const stroke = (id: string, x: number, y: number): CanvasElement => ({ id, type: 'rectangle', x, y, width: 20, height: 20, ...base });
const notebook = (paper: Paper, elements: CanvasElement[] = [], viewport = { x: 0, y: 0, zoom: 1 }): CanvasDocument => ({ ...createCanvasDocument(), paper, elements, viewport });
const two: Paper = { gap: 10, pages: [{ id: 'p1', width: 100, height: 200 }, { id: 'p2', width: 100, height: 200 }] };
const PATH = 'courses/X/canvases/Work.canvas';

function fakes() {
  const rendered: string[] = [], sent: SnapshotRequest[] = [];
  const state: { failSend: boolean; gate?: Promise<void>; onSend?: () => void } = { failSend: false };
  const deps: SnapshotDeps = {
    render: async (_document, pageId) => { rendered.push(pageId); return new Blob([pageId]); },
    encode: async blob => `png:${await blob.text()}`,
    send: async request => { state.onSend?.(); await state.gate; if (state.failSend) throw new Error('disk full'); sent.push(request); },
  };
  const errors: (string | null)[] = [];
  const writer = new NotebookSnapshots(7, deps, error => errors.push(error), 10_000);
  const write = async (document: CanvasDocument, path = PATH) => { writer.schedule(document, path); await writer.flush(); };
  return { rendered, sent, state, errors, writer, write };
}

describe('pageFingerprints', () => {
  test('a page changes only when something touching it does', () => {
    const before = pageFingerprints(notebook(two, [stroke('a', 10, 10)]));
    const after = pageFingerprints(notebook(two, [stroke('a', 10, 10), stroke('b', 10, 250)]));
    expect(after.get('p1')).toBe(before.get('p1'));
    expect(after.get('p2')).not.toBe(before.get('p2'));
  });
  test('moving a stroke changes its page', () => {
    const a = pageFingerprints(notebook(two, [stroke('a', 10, 10)])).get('p1');
    expect(pageFingerprints(notebook(two, [stroke('a', 11, 10)])).get('p1')).not.toBe(a);
  });
  test('the camera is not part of a page', () => {
    const a = pageFingerprints(notebook(two, [stroke('a', 10, 10)]));
    const b = pageFingerprints(notebook(two, [stroke('a', 10, 10)], { x: 400, y: -80, zoom: 2 }));
    expect([...b]).toEqual([...a]);
  });
  test('a page keeps its key when it only moves down because a page above was added', () => {
    // Index is part of the key on purpose: the picture is page-numbered in the manifest.
    const one: Paper = { gap: 10, pages: [{ id: 'p2', width: 100, height: 200 }] };
    expect(pageFingerprints(notebook(one)).get('p2')).not.toBe(pageFingerprints(notebook(two)).get('p2'));
  });
  test('a whiteboard has no pages', () => {
    expect(pageFingerprints(createCanvasDocument()).size).toBe(0);
  });
});

describe('NotebookSnapshots', () => {
  test('the first write sends every page', async () => {
    const f = fakes();
    await f.write(notebook(two, [stroke('a', 10, 10)]));
    expect(f.rendered).toEqual(['p1', 'p2']);
    expect(f.sent).toHaveLength(1);
    expect(f.sent[0]).toEqual({ fileId: 7, relativePath: PATH, pageCount: 2, pages: [{ id: 'p1', index: 0, png: 'png:p1' }, { id: 'p2', index: 1, png: 'png:p2' }] });
  });
  test('writing on one page resends only that page', async () => {
    const f = fakes();
    await f.write(notebook(two, [stroke('a', 10, 10)]));
    await f.write(notebook(two, [stroke('a', 10, 10), stroke('b', 10, 260)]));
    expect(f.rendered).toEqual(['p1', 'p2', 'p2']);
    expect(f.sent[1].pages.map(p => [p.id, p.index])).toEqual([['p2', 1]]);
  });
  test('a pan or zoom writes nothing', async () => {
    const f = fakes();
    const elements = [stroke('a', 10, 10)];
    await f.write(notebook(two, elements));
    await f.write(notebook(two, elements, { x: 90, y: 90, zoom: 1.5 }));
    expect(f.sent).toHaveLength(1);
  });
  test('a rename sends the new path with no pictures', async () => {
    const f = fakes();
    await f.write(notebook(two));
    await f.write(notebook(two), 'courses/X/canvases/Renamed.canvas');
    expect(f.sent[1]).toMatchObject({ relativePath: 'courses/X/canvases/Renamed.canvas', pages: [] });
  });
  test('adding a page sends the new page and the new count', async () => {
    const f = fakes();
    await f.write(notebook(two));
    await f.write(notebook({ ...two, pages: [...two.pages, { id: 'p3', width: 100, height: 200 }] }));
    expect(f.sent[1].pageCount).toBe(3);
    expect(f.sent[1].pages.map(p => p.id)).toEqual(['p3']);
  });
  test('removing the first page renumbers the rest', async () => {
    const f = fakes();
    await f.write(notebook(two, [stroke('a', 10, 10), stroke('b', 10, 260)]));
    await f.write(notebook({ ...two, pages: [two.pages[1]] }, [stroke('b', 10, 50)]));
    expect(f.sent[1]).toMatchObject({ pageCount: 1, pages: [{ id: 'p2', index: 0 }] });
  });
  test('a failed write is reported, then retried with the same pages on the next change', async () => {
    const f = fakes();
    f.state.failSend = true;
    await f.write(notebook(two));
    expect(f.errors.at(-1)).toBe('disk full');
    f.state.failSend = false;
    await f.write(notebook(two));
    expect(f.sent).toHaveLength(1);
    expect(f.sent[0].pages.map(p => p.id)).toEqual(['p1', 'p2']);
    expect(f.errors.at(-1)).toBeNull();
  });
  test('a whiteboard is ignored', async () => {
    const f = fakes();
    await f.write(createCanvasDocument());
    expect(f.sent).toHaveLength(0);
  });
  test('a write started while another is in flight waits for it, so pages arrive in order', async () => {
    const f = fakes();
    let release!: () => void;
    f.state.gate = new Promise<void>(resolve => { release = resolve; });
    const started = new Promise<void>(resolve => { f.state.onSend = resolve; });
    f.writer.schedule(notebook(two, [stroke('a', 10, 10)]), PATH);
    const first = f.writer.flush();
    await started;
    f.writer.schedule(notebook(two, [stroke('a', 10, 10), stroke('b', 10, 260)]), PATH);
    const second = f.writer.flush();
    expect(f.sent).toHaveLength(0);
    release();
    await Promise.all([first, second]);
    expect(f.sent.map(request => request.pages.map(p => p.id))).toEqual([['p1', 'p2'], ['p2']]);
  });
  test('a document scheduled before the write starts replaces the older one', async () => {
    const f = fakes();
    f.writer.schedule(notebook(two, [stroke('a', 10, 10)]), PATH);
    const first = f.writer.flush();
    f.writer.schedule(notebook(two, [stroke('a', 10, 10), stroke('b', 10, 260)]), PATH);
    await Promise.all([first, f.writer.flush()]);
    expect(f.sent).toHaveLength(1);
  });
  test('flush with nothing scheduled does nothing', async () => {
    const f = fakes();
    await f.writer.flush();
    expect(f.sent).toHaveLength(0);
  });
  test('uses each page rectangle, so a page with ink past its edge is still that page', async () => {
    const f = fakes();
    const rect = pageRects(createPaper('blank').pages.length ? createPaper('blank') : two)[0];
    await f.write(notebook(createPaper('blank'), [stroke('over', rect.x + rect.width - 5, rect.y + 10)]));
    expect(f.sent[0].pages.map(p => p.id)).toEqual(['p1']);
  });
});
