import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import { createHarness, pointer } from './harness';

const look = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const stroke = (id: string, x: number, y: number): CanvasElement => ({ id, type: 'freedraw', x, y, width: 40, height: 0, ...look, points: [{ x: 0, y: 0 }, { x: 20, y: 0 }, { x: 40, y: 0 }] });
const scene = () => ({ ...createCanvasDocument(), elements: [stroke('near', 20, 20), stroke('far', 300, 300)] });

describe('lasso tool', () => {
  function loop(h: ReturnType<typeof createHarness>) {
    h.editor.pointerDown(pointer(1, 'pen', 5, 5));
    for (const [x, y] of [[90, 5], [90, 40], [5, 40]]) h.editor.pointerMove(pointer(1, 'pen', x, y));
    h.editor.pointerUp(pointer(1, 'pen', 5, 6));
    h.render();
  }
  test('a loop selects the ink inside it and the tool stays', () => {
    const h = createHarness();
    h.render(scene()); h.editor.setTool('lasso'); h.render();
    loop(h);
    expect(h.editor.selection).toEqual(['near']);
    expect(h.editor.tool).toBe('lasso');
    expect(h.published).toHaveLength(0);
  });
  test('dragging inside the selection moves it as one undo step, and a loop elsewhere replaces it', () => {
    const h = createHarness();
    h.render(scene()); h.editor.setTool('lasso'); h.render();
    loop(h);
    h.editor.pointerDown(pointer(2, 'pen', 40, 21)); h.editor.pointerMove(pointer(2, 'pen', 50, 41)); h.editor.pointerUp(pointer(2, 'pen', 50, 41));
    h.render();
    expect(h.editor.documentRef.current.elements.find(element => element.id === 'near')).toMatchObject({ x: 30, y: 40 });
    expect(h.editor.selection).toEqual(['near']);
    h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements.find(element => element.id === 'near')).toMatchObject({ x: 20, y: 20 });
    h.editor.pointerDown(pointer(3, 'pen', 200, 200)); h.editor.pointerUp(pointer(3, 'pen', 200, 200));
    h.render();
    expect(h.editor.selection).toEqual([]);
  });
});

describe('partial eraser', () => {
  test('a drag across a stroke cuts it in two as one undo step', () => {
    const h = createHarness({ eraserMode: 'partial', eraserSize: 8 });
    h.render(scene()); h.editor.setTool('eraser'); h.render();
    h.editor.pointerDown(pointer(1, 'pen', 40, 5)); h.editor.pointerMove(pointer(1, 'pen', 40, 35)); h.editor.pointerUp(pointer(1, 'pen', 40, 35));
    h.render();
    const pieces = h.editor.documentRef.current.elements.filter(element => element.x === 20);
    expect(pieces).toHaveLength(2);
    expect(pieces[0].id).toBe('near');
    expect(pieces[0].points!.at(-1)!.x).toBeLessThan(20);
    expect(pieces[1].points![0].x).toBeGreaterThan(20);
    expect(h.published).toHaveLength(1);
    h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements).toEqual(scene().elements);
  });
  test('the whole-stroke eraser still removes the stroke it touches', () => {
    const h = createHarness({ eraserMode: 'stroke' });
    h.render(scene()); h.editor.setTool('eraser'); h.render();
    h.editor.pointerDown(pointer(1, 'pen', 40, 20)); h.editor.pointerUp(pointer(1, 'pen', 40, 20));
    h.render();
    expect(h.editor.documentRef.current.elements.map(element => element.id)).toEqual(['far']);
  });
});
