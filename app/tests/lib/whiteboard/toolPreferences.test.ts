import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { DEFAULT_TOOL_PREFERENCES, loadCanvasToolPreferences, saveCanvasToolPreferences } from '../../../src/lib/canvas/toolPreferences';

const KEY = 'oculus-canvas-tool-preferences-v1';
let store: Map<string, string>;
const previous = (globalThis as { localStorage?: Storage }).localStorage;
beforeEach(() => {
  store = new Map();
  Object.assign(globalThis, { localStorage: { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => { store.set(key, value); } } });
});
afterEach(() => { Object.assign(globalThis, { localStorage: previous }); });

describe('device drawing preferences', () => {
  test('nothing saved gives the defaults', () => {
    expect(loadCanvasToolPreferences()).toEqual({ eraserAutoReturn: false, eraserMode: 'stroke', eraserSize: 'medium' });
  });
  test('a save reads back', () => {
    const preferences = { eraserAutoReturn: true, eraserMode: 'partial', eraserSize: 'large' } as const;
    saveCanvasToolPreferences(preferences);
    expect(loadCanvasToolPreferences()).toEqual(preferences);
  });
  test('a value saved before the eraser fields existed keeps its switch and defaults the rest', () => {
    store.set(KEY, JSON.stringify({ eraserAutoReturn: true }));
    expect(loadCanvasToolPreferences()).toEqual({ ...DEFAULT_TOOL_PREFERENCES, eraserAutoReturn: true });
  });
  test('junk falls back field by field', () => {
    for (const junk of ['not json', 'null', '[1,2]', '42', JSON.stringify({ eraserAutoReturn: 'yes', eraserMode: 'everything', eraserSize: 'toString' })]) {
      store.set(KEY, junk);
      expect(loadCanvasToolPreferences()).toEqual(DEFAULT_TOOL_PREFERENCES);
    }
    store.set(KEY, JSON.stringify({ eraserMode: 'partial', eraserSize: 99 }));
    expect(loadCanvasToolPreferences()).toEqual({ ...DEFAULT_TOOL_PREFERENCES, eraserMode: 'partial' });
  });
  test('storage that throws gives the defaults', () => {
    Object.assign(globalThis, { localStorage: { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('denied'); } } });
    saveCanvasToolPreferences({ ...DEFAULT_TOOL_PREFERENCES, eraserMode: 'partial' });
    expect(loadCanvasToolPreferences()).toEqual(DEFAULT_TOOL_PREFERENCES);
  });
});
