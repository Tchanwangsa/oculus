import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import { createHarness, pointer, type Harness } from './harness';

const look = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
/** A diagonal stroke 60 by 30, so its selection frame has room inside clear of the resize handles. */
const stroke = (id: string, x: number, y: number): CanvasElement => ({ id, type: 'freedraw', x, y, width: 60, height: 30, ...look, points: [{ x: 0, y: 0 }, { x: 30, y: 15 }, { x: 60, y: 30 }] });
const scene = () => ({ ...createCanvasDocument(), elements: [stroke('near', 20, 20), stroke('far', 300, 300)] });

function lassoed() {
  const h = createHarness();
  h.render(scene()); h.editor.setTool('lasso'); h.render();
  return h;
}
function loop(h: Harness, corners: [number, number][], id = 1, extra: Record<string, unknown> = {}) {
  const [[x, y], ...rest] = corners;
  h.editor.pointerDown(pointer(id, 'pen', x, y, extra));
  for (const [cx, cy] of rest) h.editor.pointerMove(pointer(id, 'pen', cx, cy, extra));
  h.editor.pointerUp(pointer(id, 'pen', x, y + 1, extra));
  h.render();
}
const aroundNear: [number, number][] = [[5, 5], [100, 5], [100, 60], [5, 60]];
const aroundFar: [number, number][] = [[290, 290], [380, 290], [380, 340], [290, 340]];

describe('lasso tool', () => {
  test('a loop selects the ink inside it, the tool stays, and nothing is published', () => {
    const h = lassoed();
    loop(h, aroundNear);
    expect(h.editor.selection).toEqual(['near']);
    expect(h.editor.tool).toBe('lasso');
    expect(h.published).toHaveLength(0);
  });

  test('a loop started on an unselected stroke draws a loop rather than moving it', () => {
    const h = lassoed();
    loop(h, [[50, 35], [120, 0], [120, 80], [0, 80], [0, 0]]);
    expect(h.editor.selection).toEqual(['near']);
    expect(h.editor.documentRef.current.elements.find(element => element.id === 'near')).toMatchObject({ x: 20, y: 20 });
  });

  test('Shift adds a second loop to the selection', () => {
    const h = lassoed();
    loop(h, aroundNear);
    loop(h, aroundFar, 2, { shiftKey: true });
    expect(new Set(h.editor.selection)).toEqual(new Set(['near', 'far']));
  });

  test('dragging inside the selection moves it as one undo step, and a tap elsewhere clears it', () => {
    const h = lassoed();
    loop(h, aroundNear);
    h.editor.pointerDown(pointer(2, 'pen', 35, 30)); h.editor.pointerMove(pointer(2, 'pen', 45, 50)); h.editor.pointerUp(pointer(2, 'pen', 45, 50));
    h.render();
    expect(h.editor.documentRef.current.elements.find(element => element.id === 'near')).toMatchObject({ x: 30, y: 40 });
    expect(h.editor.selection).toEqual(['near']);
    h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements.find(element => element.id === 'near')).toMatchObject({ x: 20, y: 20 });
    h.editor.pointerDown(pointer(3, 'pen', 200, 200)); h.editor.pointerUp(pointer(3, 'pen', 200, 200));
    h.render();
    expect(h.editor.selection).toEqual([]);
  });

  test('choosing the lasso keeps the selection Select made', () => {
    const h = createHarness();
    h.render(scene());
    h.editor.pointerDown(pointer(1, 'mouse', 50, 35)); h.editor.pointerUp(pointer(1, 'mouse', 50, 35)); h.render();
    expect(h.editor.selection).toEqual(['near']);
    h.editor.setTool('lasso'); h.render();
    expect(h.editor.selection).toEqual(['near']);
  });
});
