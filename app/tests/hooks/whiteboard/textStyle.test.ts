import { describe, expect, test } from 'bun:test';
import { createCanvasDocument, type CanvasElement } from '@/lib/canvas/model';
import { createHarness, pointer } from './harness';

function startText() {
  const h = createHarness();
  h.editor.setTool('text'); h.render();
  h.editor.pointerDown(pointer(1, 'mouse', 300, 200)); h.render();
  return h;
}

describe('formatting an open text draft', () => {
  test('an empty draft accepts formatting before the first character and commits the latest typed text', () => {
    const h = startText();
    h.editor.changeStyle({ fontSize: 36, fontFamily: 'serif', stroke: '#ff0000', textAlign: 'center' }); h.render();
    expect(h.editor.textDraft?.element).toMatchObject({ fontSize: 36, fontFamily: 'serif', stroke: '#ff0000', textAlign: 'center' });
    expect(h.published).toHaveLength(0);
    h.editor.editText('First character');
    h.editor.commitText(); h.render();
    expect(h.editor.documentRef.current.elements[0]).toMatchObject({ text: 'First character', fontSize: 36, fontFamily: 'serif', stroke: '#ff0000', textAlign: 'center' });
    expect(h.editor.textDraft).toBeNull();
  });

  test('opacity previews keep an empty draft open and never publish an empty element', () => {
    const h = startText();
    h.editor.previewStyle({ opacity: 0.7 }); h.render();
    h.editor.previewStyle({ opacity: 0.4 }); h.render();
    expect(h.editor.textDraft?.element.opacity).toBe(0.4);
    expect(h.published).toHaveLength(0);
    h.editor.changeStyle({ opacity: 0.4 }); h.render();
    expect(h.editor.textDraft).not.toBeNull();
    h.editor.editText('Faded'); h.editor.commitText(); h.render();
    expect(h.editor.documentRef.current.elements[0]).toMatchObject({ text: 'Faded', opacity: 0.4 });
  });

  test('opacity previews on an existing label commit once and undo restores its opacity', () => {
    const h = createHarness();
    const shape: CanvasElement = { id: 'shape', type: 'rectangle', x: 300, y: 200, width: 120, height: 80, stroke: '#000000', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 1, text: 'Label' };
    h.render({ ...createCanvasDocument(), elements: [shape] });
    h.editor.setTool('text'); h.render();
    h.editor.pointerDown(pointer(1, 'mouse', 330, 230)); h.render();
    h.editor.previewStyle({ opacity: 0.7 }); h.render();
    h.editor.previewStyle({ opacity: 0.3 }); h.render();
    expect(h.published).toHaveLength(0);
    expect(h.editor.textDraft?.kind).toBe('label');
    h.editor.changeStyle({ opacity: 0.3 }); h.render();
    expect(h.published).toHaveLength(1);
    expect(h.editor.documentRef.current.elements[0].opacity).toBe(0.3);
    h.editor.commitText(); h.render();
    h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements[0]).toEqual(shape);
    expect(h.editor.canUndo).toBe(false);
  });

  test('leaving an empty formatted draft discards it without adding undo history', () => {
    const h = startText();
    h.editor.changeStyle({ fontSize: 48 }); h.render();
    h.editor.commitText(); h.render();
    expect(h.editor.textDraft).toBeNull();
    expect(h.editor.documentRef.current.elements).toEqual([]);
    expect(h.editor.canUndo).toBe(false);
    expect(h.published).toHaveLength(0);
  });
});
