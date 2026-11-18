import { describe, expect, test } from 'bun:test';
import { createHarness, pointer, type Harness } from './harness';

function draw(h: Harness, id: number) {
  h.editor.pointerDown(pointer(id)); h.editor.pointerMove(pointer(id, 'pen', 40, 30)); h.editor.pointerUp(pointer(id, 'pen', 60, 40));
  h.render();
  return h.editor.documentRef.current.elements.at(-1)!;
}
function tool(h: Harness, name: Parameters<Harness['editor']['setTool']>[0]) { h.editor.setTool(name); h.render(); }

describe('drawing with presets', () => {
  test('the pen draws with its preset in use, and switching preset switches pen, colour and width together', () => {
    const h = createHarness();
    tool(h, 'freedraw');
    expect(draw(h, 1)).toMatchObject({ stroke: '#292b32', strokeWidth: 2 });
    expect(h.editor.inkPresets!.items).toHaveLength(4);
    h.editor.inkPresets!.select(3); h.render();
    expect(draw(h, 2)).toMatchObject({ pen: 'fountain', stroke: '#292b32', strokeWidth: 4 });
    expect(h.editor.inkPresets!.active).toBe(3);
  });

  test('changing the look edits the preset in use and leaves the others and the shapes alone', () => {
    const h = createHarness();
    tool(h, 'freedraw');
    h.editor.inkPresets!.select(1); h.render();
    h.editor.changeStyle({ stroke: '#00aa00', strokeWidth: 7 }); h.render();
    expect(draw(h, 1)).toMatchObject({ stroke: '#00aa00', strokeWidth: 7 });
    h.editor.inkPresets!.select(0); h.render();
    expect(draw(h, 2)).toMatchObject({ stroke: '#292b32', strokeWidth: 2 });
    tool(h, 'rectangle');
    expect(h.editor.inspector.values).toMatchObject({ stroke: '#292b32', strokeWidth: 2 });
    expect(h.editor.inkPresets).toBeNull();
  });

  test('the highlighter has its own presets, untouched by the pen', () => {
    const h = createHarness();
    tool(h, 'freedraw');
    h.editor.changeStyle({ stroke: '#123456' }); h.render();
    tool(h, 'highlighter');
    expect(h.editor.inkPresets!.items).toHaveLength(3);
    expect(draw(h, 1)).toMatchObject({ pen: 'roundMarker', stroke: '#ffe14d', strokeWidth: 16, opacity: 0.4 });
    h.editor.inkPresets!.select(2); h.render();
    expect(draw(h, 2)).toMatchObject({ pen: 'roundMarker', stroke: '#ff8fc7' });
  });

  test('restyling a selected stroke changes that stroke, not a preset', () => {
    const h = createHarness();
    tool(h, 'freedraw');
    const first = draw(h, 1);
    tool(h, 'select');
    h.editor.pointerDown(pointer(2, 'mouse', 10, 10)); h.editor.pointerUp(pointer(2, 'mouse', 10, 10)); h.render();
    expect(h.editor.selection).toEqual([first.id]);
    h.editor.changeStyle({ stroke: '#ff0000' }); h.render();
    expect(h.editor.documentRef.current.elements[0].stroke).toBe('#ff0000');
    tool(h, 'freedraw');
    expect(draw(h, 3)).toMatchObject({ stroke: '#292b32' });
  });
});
