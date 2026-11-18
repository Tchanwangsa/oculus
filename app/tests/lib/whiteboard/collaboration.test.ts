import { describe, expect, spyOn, test } from 'bun:test';
import { createWhiteboardDocument, type WhiteboardElement } from '../../../src/lib/whiteboard/model';
import { applyWhiteboardTransaction, diffWhiteboardContent, hasWhiteboardChanges, sameValue } from '../../../src/lib/whiteboard/collaboration';
import { reconcileClientDocument } from '../../../src/lib/whiteboard/clientReconciliation';

const shape = (id: string, x = 0): WhiteboardElement => ({ id, type: 'rectangle', x, y: 20, width: 60, height: 40, stroke: '#000', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 1 });
const empty = createWhiteboardDocument();
const scene = (...elements: WhiteboardElement[]) => ({ ...empty, elements });

describe('shared drawing transactions', () => {
  test('a stroke on a PDF-sized drawing serializes only the changed element', () => {
    const image: WhiteboardElement = { ...shape('page'), type: 'image', dataUrl: 'data:image/png;base64,' + 'A'.repeat(5 * 1024 * 1024), naturalWidth: 1, naturalHeight: 1 };
    const original = scene(image);
    const stroke: WhiteboardElement = { ...shape('ink'), type: 'freedraw', points: [{ x: 0, y: 0 }, { x: 1, y: 1 }] };
    const stringify = spyOn(JSON, 'stringify');
    try {
      const result = applyWhiteboardTransaction(original, { edits: [{ id: stroke.id, before: null, after: stroke }] });
      expect(result.elements[0]).toBe(image);
      expect(result.elements[1]).toBe(stroke);
      expect(stringify.mock.calls.length).toBe(1);
      for (const call of stringify.mock.results) expect((call.value as string).length).toBeLessThan(100_000);
    } finally { stringify.mockRestore(); }
  });
  test('simultaneous additions based on the same snapshot survive in either arrival order', () => {
    const a = diffWhiteboardContent(empty, scene(shape('alice'))), b = diffWhiteboardContent(empty, scene(shape('bob')));
    for (const transactions of [[a, b], [b, a]]) {
      const result = transactions.reduce(applyWhiteboardTransaction, empty);
      expect(new Set(result.elements.map(el => el.id))).toEqual(new Set(['alice', 'bob']));
    }
  });
  test('pan and zoom do not become shared changes', () => {
    expect(hasWhiteboardChanges(diffWhiteboardContent(empty, { ...empty, viewport: { x: 300, y: -200, zoom: 3 } }))).toBe(false);
    const viewport = { x: 700, y: 20, zoom: 4 };
    const result = applyWhiteboardTransaction({ ...empty, viewport }, diffWhiteboardContent(empty, scene(shape('alice'))));
    expect(result.viewport).toBe(viewport);
  });
  test('undo removes only the local addition and keeps later collaborator strokes', () => {
    const own = scene(shape('alice'));
    const both = applyWhiteboardTransaction(own, diffWhiteboardContent(empty, scene(shape('bob'))));
    const undone = applyWhiteboardTransaction(both, diffWhiteboardContent(own, empty));
    expect(undone.elements.map(el => el.id)).toEqual(['bob']);
    expect(applyWhiteboardTransaction(undone, diffWhiteboardContent(empty, own)).elements.map(el => el.id)).toEqual(['bob', 'alice']);
  });
  test('edits to separate objects merge from the same old scene', () => {
    const original = scene(shape('alice'), shape('bob'));
    const a = diffWhiteboardContent(original, scene(shape('alice', 10), shape('bob')));
    const b = diffWhiteboardContent(original, scene(shape('alice'), shape('bob', 20)));
    expect(applyWhiteboardTransaction(applyWhiteboardTransaction(original, a), b).elements.map(el => el.x)).toEqual([10, 20]);
  });
  test('conflicting edits fail atomically without overwriting a collaborator', () => {
    const original = scene(shape('alice'));
    const moved = applyWhiteboardTransaction(original, diffWhiteboardContent(original, scene(shape('alice', 10))));
    const competing = diffWhiteboardContent(original, scene(shape('alice', 20), shape('bob')));
    expect(() => applyWhiteboardTransaction(moved, competing)).toThrow('same shape');
    expect(moved.elements).toEqual([shape('alice', 10)]);
  });
  test('undo refuses to remove a local shape that someone else changed', () => {
    const own = scene(shape('alice'));
    expect(() => applyWhiteboardTransaction(scene(shape('alice', 50)), diffWhiteboardContent(own, empty))).toThrow('same shape');
  });
  test('a delayed edit cannot resurrect a deleted shape', () => {
    const original = scene(shape('alice'));
    expect(() => applyWhiteboardTransaction(empty, diffWhiteboardContent(original, scene(shape('alice', 50))))).toThrow('same shape');
  });
  test('reordering existing shapes keeps concurrently appended shapes', () => {
    const original = scene(shape('a'), shape('b'));
    const result = applyWhiteboardTransaction(scene(shape('a'), shape('b'), shape('remote')), diffWhiteboardContent(original, scene(shape('b'), shape('a'))));
    expect(result.elements.map(el => el.id)).toEqual(['b', 'a', 'remote']);
  });
  test('retrying an already applied transaction is harmless', () => {
    const change = diffWhiteboardContent(empty, scene(shape('a')));
    const first = applyWhiteboardTransaction(empty, change);
    expect(applyWhiteboardTransaction(first, change)).toEqual(first);
  });
  test('rejects viewport injection, malformed shapes, duplicate IDs and mismatched layer order', () => {
    expect(() => applyWhiteboardTransaction(empty, { edits: [], viewport: { x: 0, y: 0, zoom: 2 } })).toThrow();
    expect(() => applyWhiteboardTransaction(empty, { edits: [{ id: 'a', before: null, after: { id: 'a' } }] })).toThrow();
    const edit = { id: 'a', before: null, after: shape('a') };
    expect(() => applyWhiteboardTransaction(empty, { edits: [edit, edit] })).toThrow();
    expect(() => applyWhiteboardTransaction(empty, { edits: [], order: { before: ['a'], after: ['b'] } })).toThrow();
  });
  test('conflict checks ignore object-key order and retain unknown element fields', () => {
    const a = { ...shape('a'), extension: { second: 2, first: 1 } };
    const b = { extension: { first: 1, second: 2 }, ...shape('a') };
    expect(sameValue(a, b)).toBe(true);
    const result = applyWhiteboardTransaction(scene(b), diffWhiteboardContent(scene(a), scene({ ...a, x: 25 })));
    expect(result.elements[0].extension).toEqual({ second: 2, first: 1 });
  });
});

describe('client scene reconciliation during rapid drawing', () => {
  test('a delayed acknowledgement never replaces the visible scene between rapid strokes', () => {
    let shared = createWhiteboardDocument();
    let local = shared;
    const pending = [] as ReturnType<typeof diffWhiteboardContent>[];
    for (let index = 0; index < 60; index++) {
      const before = local;
      local = { ...local, elements: [...local.elements, shape(`stroke-${index}`)] };
      pending.push(diffWhiteboardContent(before, local));
      // Pointer-up reconciliation must not install a fresh document prop.
      expect(reconcileClientDocument(shared, pending, local)).toBe(local);
      if (index % 3 === 2) {
        shared = applyWhiteboardTransaction(shared, pending.shift()!);
        // The next stroke can already be starting when this old ack arrives.
        expect(reconcileClientDocument(shared, pending, local)).toBe(local);
      }
    }
    while (pending.length) {
      shared = applyWhiteboardTransaction(shared, pending.shift()!);
      expect(reconcileClientDocument(shared, pending, local)).toBe(local);
    }
    expect(shared.elements.map(element => element.id)).toEqual(local.elements.map(element => element.id));
    expect(shared.elements).toHaveLength(60);
  });

  test('remote additions between delayed acknowledgements retain every queued local stroke and local camera', () => {
    let shared = createWhiteboardDocument();
    let local = { ...shared, viewport: { x: 250, y: -120, zoom: 2 } };
    const pending = [] as ReturnType<typeof diffWhiteboardContent>[];
    for (let index = 0; index < 30; index++) {
      const before = local;
      local = { ...local, elements: [...local.elements, shape(`local-${index}`)] };
      pending.push(diffWhiteboardContent(before, local));
      if (index % 5 === 4) {
        shared = applyWhiteboardTransaction(shared, diffWhiteboardContent(empty, scene(shape(`remote-${index}`))));
        local = reconcileClientDocument(shared, pending, local);
        shared = applyWhiteboardTransaction(shared, pending.shift()!);
        expect(reconcileClientDocument(shared, pending, local)).toBe(local);
      }
    }
    while (pending.length) {
      shared = applyWhiteboardTransaction(shared, pending.shift()!);
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
