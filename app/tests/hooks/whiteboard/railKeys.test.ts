import { describe, expect, test } from 'bun:test';
import type { CanvasTool } from '@/lib/canvas/model';
import { createHarness, key } from './harness';

describe('the rail shortcuts are each tool\'s letter', () => {
  const tools: [string, CanvasTool][] = [
    ['s', 'select'], ['l', 'lasso'], ['g', 'hand'], ['c', 'capture'], ['p', 'freedraw'], ['h', 'highlighter'], ['e', 'eraser'], ['t', 'text'],
    ['r', 'rectangle'], ['o', 'ellipse'], ['d', 'diamond'], ['a', 'arrow'], ['n', 'line'],
  ];
  for (const [letter, tool] of tools) {
    test(`${letter} chooses ${tool}`, () => {
      const h = createHarness();
      h.editor.keyDown(key(letter)); h.render();
      expect(h.editor.tool).toBe(tool);
    });
  }
  test('Shift+G toggles the grid and leaves the tool alone', () => {
    const h = createHarness();
    const before = h.editor.grid;
    h.editor.keyDown(key('g', { shift: true })); h.render();
    expect(h.editor.grid).toBe(!before);
    expect(h.editor.tool).toBe('select');
    h.editor.keyDown(key('G', { shift: true })); h.render();
    expect(h.editor.grid).toBe(before);
  });
  test('G alone is Grab and leaves the grid alone', () => {
    const h = createHarness();
    const before = h.editor.grid;
    h.editor.keyDown(key('g')); h.render();
    expect(h.editor.tool).toBe('hand');
    expect(h.editor.grid).toBe(before);
  });
  test('the copy shortcut is not the Capture key', () => {
    const h = createHarness();
    h.editor.keyDown(key('c', { meta: true })); h.render();
    expect(h.editor.tool).toBe('select');
  });
  test('V is not a tool key', () => {
    const h = createHarness();
    h.editor.setTool('freedraw'); h.render();
    h.editor.keyDown(key('v')); h.render();
    expect(h.editor.tool).toBe('freedraw');
  });
  test('I asks the host for the image picker', () => {
    let opened = 0;
    const h = createHarness({ onInsertImage: () => { opened++; } });
    h.editor.keyDown(key('i'));
    expect(opened).toBe(1);
  });
});

describe('the rail inserts ready-made figures and selects them', () => {
  const insert = (letter: string) => {
    const h = createHarness();
    h.editor.keyDown(key(letter)); h.render();
    return h;
  };
  test('M drops a matrix of nine squares', () => {
    const h = insert('m');
    const elements = h.published.at(-1)!.elements;
    expect(elements).toHaveLength(9);
    expect(elements.every(element => element.type === 'rectangle')).toBe(true);
    expect(h.editor.selection).toHaveLength(9);
    expect(h.editor.tool).toBe('select');
  });
  test('Y drops the x and y axes, and Z adds the third', () => {
    expect(insert('y').published.at(-1)!.elements.map(element => element.type)).toEqual(['arrow', 'arrow']);
    expect(insert('z').published.at(-1)!.elements.map(element => element.type)).toEqual(['arrow', 'arrow', 'arrow']);
  });
});
