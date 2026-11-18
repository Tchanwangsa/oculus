import { beforeEach, describe, expect, test } from 'bun:test';
import type { CanvasDocument, CanvasElement } from '@/lib/canvas/model';
import { createHarness, key, pointer, type Editor, type Harness } from './harness';

let h: Harness;
const editor = () => h.editor;
const last = () => h.published[h.published.length - 1];
const ids = (document: CanvasDocument = editor().documentRef.current) => document.elements.map(element => element.id);
function select(tool: Parameters<Editor['setTool']>[0]) { editor().setTool(tool); h.render(); }
function drag(from: [number, number], to: [number, number], id = 1) {
  editor().pointerDown(pointer(id, 'mouse', ...from));
  editor().pointerMove(pointer(id, 'mouse', ...to));
  editor().pointerUp(pointer(id, 'mouse', ...to));
  h.render();
}
const undo = () => { editor().history(); h.render(); };
const redo = () => { editor().history(true); h.render(); };
/** Another device's stroke, as a host would merge it: a fresh document object. */
function collaboratorStroke(base: CanvasDocument, id = 'theirs'): CanvasDocument {
  const element: CanvasElement = { id, type: 'freedraw', x: 300, y: 300, width: 10, height: 10, points: [{ x: 0, y: 0 }, { x: 10, y: 10 }], stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 1 };
  return { ...structuredClone(base), elements: [...structuredClone(base.elements), element] };
}
beforeEach(() => { h = createHarness(); select('rectangle'); });

describe('undo and redo on one device', () => {
  test('step through each edit and leave the camera where it is', () => {
    drag([10, 10], [40, 40]); select('rectangle'); drag([100, 10], [140, 40]); select('rectangle'); drag([200, 10], [240, 40]);
    const [a, b] = ids();
    expect(editor().canUndo).toBe(true);
    editor().zoom(2); h.render();
    const camera = editor().documentRef.current.viewport;
    undo(); expect(ids()).toEqual([a, b]);
    undo(); expect(ids()).toEqual([a]);
    expect(editor().documentRef.current.viewport).toEqual(camera);
    redo(); expect(ids()).toEqual([a, b]);
    expect(editor().canRedo).toBe(true);
    undo(); undo(); expect(ids()).toEqual([]);
    expect(editor().canUndo).toBe(false);
    undo(); expect(ids()).toEqual([]);
    redo(); redo(); redo(); expect(ids()).toHaveLength(3);
    expect(editor().canRedo).toBe(false);
  });
  test('a new edit clears redo', () => {
    drag([10, 10], [40, 40]); undo();
    expect(editor().canRedo).toBe(true);
    select('rectangle'); drag([100, 10], [140, 40]);
    expect(editor().canRedo).toBe(false);
  });
  test('undoing a delete puts the shape back at its layer', () => {
    drag([10, 10], [40, 40]); select('rectangle'); drag([100, 10], [140, 40]); select('rectangle'); drag([200, 10], [240, 40]);
    const order = ids();
    select('select'); drag([100, 25], [100, 25]);
    expect(editor().selection).toEqual([order[1]]);
    editor().keyDown(key('Delete')); h.render();
    expect(ids()).toEqual([order[0], order[2]]);
    undo(); expect(ids()).toEqual(order);
    redo(); expect(ids()).toEqual([order[0], order[2]]);
    undo(); expect(ids()).toEqual(order);
  });
  test('undo waits while a gesture is in progress', () => {
    drag([10, 10], [40, 40]); select('rectangle');
    editor().pointerDown(pointer(1, 'mouse', 100, 100)); editor().pointerMove(pointer(1, 'mouse', 140, 140));
    editor().history(); h.render();
    expect(ids()).toHaveLength(1);
    editor().pointerUp(pointer(1, 'mouse', 140, 140)); h.render();
    expect(ids()).toHaveLength(2);
    undo(); expect(ids()).toHaveLength(1);
  });
  test('a layer move is one undo step', () => {
    drag([10, 10], [40, 40]); select('rectangle'); drag([100, 10], [140, 40]);
    const order = ids();
    select('select'); drag([10, 25], [10, 25]);
    editor().reorder('front'); h.render();
    expect(ids()).toEqual([order[1], order[0]]);
    undo(); expect(ids()).toEqual(order);
    redo(); expect(ids()).toEqual([order[1], order[0]]);
  });
});

describe('undo beside a collaborator', () => {
  test('undo removes only this pane’s stroke and keeps a later collaborator stroke; redo puts it back beneath', () => {
    drag([10, 10], [40, 40]);
    const [mine] = ids();
    h.render(collaboratorStroke(last()));
    expect(ids()).toEqual([mine, 'theirs']);
    undo();
    expect(ids()).toEqual(['theirs']);
    expect(editor().historyError).toBeNull();
    redo();
    expect(ids()).toEqual([mine, 'theirs']);
  });
  test('undo is refused, with a message, when someone else changed the shape since', () => {
    drag([10, 10], [40, 40]);
    const published = h.published.length;
    const moved = structuredClone(last());
    moved.elements[0] = { ...moved.elements[0], x: 500 };
    h.render(moved);
    undo();
    expect(editor().documentRef.current.elements[0].x).toBe(500);
    expect(h.published).toHaveLength(published);
    expect(editor().historyError).toMatch(/Undo skipped/);
    expect(editor().canUndo).toBe(false);
    editor().clearHistoryError(); h.render();
    expect(editor().historyError).toBeNull();
  });
});

describe('a document from outside the pane', () => {
  test('does not cancel a freedraw stroke in progress', () => {
    select('freedraw');
    editor().pointerDown(pointer(1, 'pen', 10, 10)); editor().pointerMove(pointer(1, 'pen', 20, 20));
    h.render(collaboratorStroke(editor().documentRef.current));
    editor().pointerMove(pointer(1, 'pen', 30, 30)); editor().pointerUp(pointer(1, 'pen', 40, 40));
    const document = last();
    expect(document.elements).toHaveLength(2);
    expect(document.elements[0].id).toBe('theirs');
    expect(document.elements[1]).toMatchObject({ type: 'freedraw', x: 10, y: 10, width: 30, height: 30 });
  });
  test('keeps undo history, the camera and the selection of shapes that remain', () => {
    drag([10, 10], [40, 40]); select('rectangle'); drag([100, 10], [140, 40]);
    const [a, b] = ids();
    editor().zoom(2); h.render();
    const camera = editor().documentRef.current.viewport;
    select('select'); editor().keyDown(key('a', { meta: true })); h.render();
    expect(editor().selection).toEqual([a, b]);
    const remote = structuredClone(last());
    remote.elements = remote.elements.filter(element => element.id !== a);
    remote.viewport = { x: 0, y: 0, zoom: 1 };
    h.render(collaboratorStroke(remote)); h.render();
    expect(editor().selection).toEqual([b]);
    expect(editor().documentRef.current.viewport).toEqual(camera);
    undo();
    expect(ids()).toEqual(['theirs']);
  });
  test('leaves the text being written open', () => {
    select('text');
    editor().pointerDown(pointer(1, 'mouse', 50, 50)); h.render();
    expect(editor().textDraft).not.toBeNull();
    h.render(collaboratorStroke(editor().documentRef.current));
    expect(editor().textDraft).not.toBeNull();
    editor().commitText('hello'); h.render();
    expect(last().elements.map(element => element.type)).toEqual(['freedraw', 'text']);
  });
  test('a move continues over the new scene and never brings back a shape someone removed', () => {
    drag([10, 10], [40, 40]); select('rectangle'); drag([100, 10], [140, 40]);
    const [a, b] = ids();
    select('select'); editor().keyDown(key('a', { meta: true })); h.render();
    editor().pointerDown(pointer(1, 'mouse', 25, 10)); editor().pointerMove(pointer(1, 'mouse', 35, 10));
    const remote = structuredClone(last());
    remote.elements = remote.elements.filter(element => element.id !== a);
    h.render(remote);
    editor().pointerMove(pointer(1, 'mouse', 45, 10)); editor().pointerUp(pointer(1, 'mouse', 45, 10)); h.render();
    expect(ids()).toEqual([b]);
    expect(editor().documentRef.current.elements[0].x).toBe(120);
  });
  test('an identical document with a fresh identity changes nothing', () => {
    drag([10, 10], [40, 40]);
    const count = h.published.length;
    h.render(structuredClone(last()));
    expect(editor().canUndo).toBe(true);
    expect(h.published).toHaveLength(count);
  });
});

describe('the camera', () => {
  test('without a viewport callback, pan and zoom are saved in the document', async () => {
    select('hand');
    drag([100, 100], [130, 120]);
    expect(last().viewport).toMatchObject({ x: 30, y: 20 });
    expect(last().elements).toEqual([]);
    const count = h.published.length;
    editor().zoom(2); h.render();
    expect(h.published).toHaveLength(count);
    await Bun.sleep(220);
    expect(h.published).toHaveLength(count + 1);
    expect(last().viewport.zoom).toBe(2);
  });
  test('with a viewport callback, pan and zoom go only there', async () => {
    h = createHarness({ localViewport: true });
    select('hand');
    drag([100, 100], [130, 120]);
    editor().zoom(2); h.render();
    await Bun.sleep(220);
    expect(h.published).toHaveLength(0);
    expect(h.views[h.views.length - 1]).toMatchObject({ zoom: 2 });
    expect(h.views).toContainEqual({ x: 30, y: 20, zoom: 1 });
    expect(editor().zoomPercent).toBe(200);
    select('rectangle'); drag([10, 10], [40, 40]);
    expect(h.published).toHaveLength(1);
  });
  test('Escape during a pan returns the camera to where it started', () => {
    h = createHarness({ localViewport: true });
    select('hand');
    editor().pointerDown(pointer(1, 'mouse', 100, 100)); editor().pointerMove(pointer(1, 'mouse', 150, 150));
    expect(editor().documentRef.current.viewport).toMatchObject({ x: 50, y: 50 });
    editor().keyDown(key('Escape')); h.render();
    expect(editor().documentRef.current.viewport).toEqual({ x: 0, y: 0, zoom: 1 });
    expect(h.views[h.views.length - 1]).toEqual({ x: 0, y: 0, zoom: 1 });
  });
});
