import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '../../../src/lib/canvas/model';
import { applyCanvasTransaction, diffCanvasContent, hasCanvasChanges, sameValue } from '../../../src/lib/canvas/collaboration';
import { reconcileClientDocument } from '../../../src/lib/canvas/clientReconciliation';

const shape = (id: string, x = 0): CanvasElement => ({ id, type: 'rectangle', x, y: 20, width: 60, height: 40, stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 1 });
const empty = createCanvasDocument();
const scene = (...elements: CanvasElement[]) => ({ ...empty, elements });

describe('shared drawing transactions', () => {
  test('simultaneous additions based on the same snapshot survive in either arrival order', () => {
    const a = diffCanvasContent(empty, scene(shape('alice'))), b = diffCanvasContent(empty, scene(shape('bob')));
    for (const transactions of [[a, b], [b, a]]) {
      const result = transactions.reduce(applyCanvasTransaction, empty);
      expect(new Set(result.elements.map(el => el.id))).toEqual(new Set(['alice', 'bob']));
    }
  });
  test('pan and zoom do not become shared changes', () => {
    expect(hasCanvasChanges(diffCanvasContent(empty, { ...empty, viewport: { x: 300, y: -200, zoom: 3 } }))).toBe(false);
    const viewport = { x: 700, y: 20, zoom: 4 };
    const result = applyCanvasTransaction({ ...empty, viewport }, diffCanvasContent(empty, scene(shape('alice'))));
    expect(result.viewport).toBe(viewport);
  });
  test('undo removes only the local addition and keeps later collaborator strokes', () => {
    const own = scene(shape('alice'));
    const both = applyCanvasTransaction(own, diffCanvasContent(empty, scene(shape('bob'))));
    const undone = applyCanvasTransaction(both, diffCanvasContent(own, empty));
    expect(undone.elements.map(el => el.id)).toEqual(['bob']);
    expect(applyCanvasTransaction(undone, diffCanvasContent(empty, own)).elements.map(el => el.id)).toEqual(['bob', 'alice']);
  });
  test('edits to separate objects merge from the same old scene', () => {
    const original = scene(shape('alice'), shape('bob'));
    const a = diffCanvasContent(original, scene(shape('alice', 10), shape('bob')));
    const b = diffCanvasContent(original, scene(shape('alice'), shape('bob', 20)));
    expect(applyCanvasTransaction(applyCanvasTransaction(original, a), b).elements.map(el => el.x)).toEqual([10, 20]);
  });
  test('conflicting edits fail atomically without overwriting a collaborator', () => {
    const original = scene(shape('alice'));
    const moved = applyCanvasTransaction(original, diffCanvasContent(original, scene(shape('alice', 10))));
    const competing = diffCanvasContent(original, scene(shape('alice', 20), shape('bob')));
    expect(() => applyCanvasTransaction(moved, competing)).toThrow('same shape');
    expect(moved.elements).toEqual([shape('alice', 10)]);
  });
  test('undo refuses to remove a local shape that someone else changed', () => {
    const own = scene(shape('alice'));
    expect(() => applyCanvasTransaction(scene(shape('alice', 50)), diffCanvasContent(own, empty))).toThrow('same shape');
  });
  test('a delayed edit cannot resurrect a deleted shape', () => {
    const original = scene(shape('alice'));
    expect(() => applyCanvasTransaction(empty, diffCanvasContent(original, scene(shape('alice', 50))))).toThrow('same shape');
  });
  test('reordering existing shapes keeps concurrently appended shapes', () => {
    const original = scene(shape('a'), shape('b'));
    const result = applyCanvasTransaction(scene(shape('a'), shape('b'), shape('remote')), diffCanvasContent(original, scene(shape('b'), shape('a'))));
    expect(result.elements.map(el => el.id)).toEqual(['b', 'a', 'remote']);
  });
  test('retrying an already applied transaction is harmless', () => {
    const change = diffCanvasContent(empty, scene(shape('a')));
    const first = applyCanvasTransaction(empty, change);
    expect(applyCanvasTransaction(first, change)).toEqual(first);
  });
  test('rejects viewport injection, malformed shapes, duplicate IDs and mismatched layer order', () => {
    expect(() => applyCanvasTransaction(empty, { edits: [], viewport: { x: 0, y: 0, zoom: 2 } })).toThrow();
    expect(() => applyCanvasTransaction(empty, { edits: [{ id: 'a', before: null, after: { id: 'a' } }] })).toThrow();
    const edit = { id: 'a', before: null, after: shape('a') };
    expect(() => applyCanvasTransaction(empty, { edits: [edit, edit] })).toThrow();
    expect(() => applyCanvasTransaction(empty, { edits: [], order: { before: ['a'], after: ['b'] } })).toThrow();
  });
  test('conflict checks ignore object-key order and retain unknown element fields', () => {
    const a = { ...shape('a'), extension: { second: 2, first: 1 } };
    const b = { extension: { first: 1, second: 2 }, ...shape('a') };
    expect(sameValue(a, b)).toBe(true);
    const result = applyCanvasTransaction(scene(b), diffCanvasContent(scene(a), scene({ ...a, x: 25 })));
    expect(result.elements[0].extension).toEqual({ second: 2, first: 1 });
  });
});

describe('client scene reconciliation during rapid drawing', () => {
  test('a delayed acknowledgement never replaces the visible scene between rapid strokes', () => {
    let shared = createCanvasDocument();
    let local = shared;
    const pending = [] as ReturnType<typeof diffCanvasContent>[];
    for (let index = 0; index < 60; index++) {
      const before = local;
      local = { ...local, elements: [...local.elements, shape(`stroke-${index}`)] };
      pending.push(diffCanvasContent(before, local));
      // Pointer-up reconciliation must not install a fresh document prop.
      expect(reconcileClientDocument(shared, pending, local)).toBe(local);
      if (index % 3 === 2) {
        shared = applyCanvasTransaction(shared, pending.shift()!);
        // The next stroke can already be starting when this old ack arrives.
        expect(reconcileClientDocument(shared, pending, local)).toBe(local);
      }
    }
    while (pending.length) {
      shared = applyCanvasTransaction(shared, pending.shift()!);
      expect(reconcileClientDocument(shared, pending, local)).toBe(local);
    }
    expect(shared.elements.map(element => element.id)).toEqual(local.elements.map(element => element.id));
    expect(shared.elements).toHaveLength(60);
  });

  test('remote additions between delayed acknowledgements retain every queued local stroke and local camera', () => {
    let shared = createCanvasDocument();
    let local = { ...shared, viewport: { x: 250, y: -120, zoom: 2 } };
    const pending = [] as ReturnType<typeof diffCanvasContent>[];
    for (let index = 0; index < 30; index++) {
      const before = local;
      local = { ...local, elements: [...local.elements, shape(`local-${index}`)] };
      pending.push(diffCanvasContent(before, local));
      if (index % 5 === 4) {
        shared = applyCanvasTransaction(shared, diffCanvasContent(empty, scene(shape(`remote-${index}`))));
        local = reconcileClientDocument(shared, pending, local);
        shared = applyCanvasTransaction(shared, pending.shift()!);
        expect(reconcileClientDocument(shared, pending, local)).toBe(local);
      }
    }
    while (pending.length) {
      shared = applyCanvasTransaction(shared, pending.shift()!);
      expect(reconcileClientDocument(shared, pending, local)).toBe(local);
    }
    expect(local.elements.filter(element => element.id.startsWith('local-'))).toHaveLength(30);
    expect(local.elements.filter(element => element.id.startsWith('remote-'))).toHaveLength(6);
    expect(local.viewport).toEqual({ x: 250, y: -120, zoom: 2 });
    expect(shared.viewport).toEqual(empty.viewport);
  });

  test('camera-only and JSON-cloned acknowledgements keep the current scene identity', () => {
    const local = { ...scene(shape('pen')), viewport: { x: 10, y: 30, zoom: 1.5 } };
    expect(reconcileClientDocument(JSON.parse(JSON.stringify(scene(shape('pen')))), [], local)).toBe(local);
    const viewport = { ...local.viewport, x: 40 };
    const moved = reconcileClientDocument(scene(shape('pen')), [], local, viewport);
    expect(moved.viewport).toBe(viewport);
    expect(moved.elements).toEqual(local.elements);
  });
});
