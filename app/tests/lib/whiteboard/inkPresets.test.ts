import { afterEach, describe, expect, test } from 'bun:test';
import { DEFAULT_INK_PRESETS, activePreset, loadInkPresets, patchPreset, saveInkPresets, selectPreset, toolPresets } from '@/lib/whiteboard/inkPresets';

// Test files share one global, so a replaced `localStorage` goes back exactly as it was: still writable, or absent.
const g = globalThis as { localStorage?: unknown };
const had = 'localStorage' in g, before = g.localStorage;
function withStorage(initial?: string) {
  const store = new Map<string, string>(initial ? [['oculus-whiteboard-ink-presets-v1', initial]] : []);
  g.localStorage = { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => { store.set(key, value); } };
  return store;
}
afterEach(() => { if (had) g.localStorage = before; else delete g.localStorage; });

describe('ink presets', () => {
  test('each tool has its own presets and its own one in use', () => {
    expect(toolPresets(DEFAULT_INK_PRESETS, 'freedraw').items).toHaveLength(4);
    expect(toolPresets(DEFAULT_INK_PRESETS, 'highlighter').items).toHaveLength(3);
    const next = selectPreset(DEFAULT_INK_PRESETS, 'freedraw', 2);
    expect(activePreset(next, 'freedraw')).toMatchObject({ pen: 'ball', stroke: '#c8483f' });
    expect(activePreset(next, 'highlighter')).toEqual(activePreset(DEFAULT_INK_PRESETS, 'highlighter'));
    expect(selectPreset(next, 'freedraw', 2)).toBe(next);
    expect(selectPreset(next, 'freedraw', 9)).toBe(next);
  });

  test('a change of look edits only the preset in use, and only its ink fields', () => {
    const next = patchPreset(selectPreset(DEFAULT_INK_PRESETS, 'freedraw', 1), 'freedraw', { stroke: '#00ff00', strokeWidth: 6, pen: 'brush', fontSize: 40 });
    expect(next.pens[1]).toEqual({ pen: 'brush', stroke: '#00ff00', strokeWidth: 6, opacity: 1 });
    expect(next.pens[0]).toBe(DEFAULT_INK_PRESETS.pens[0]);
    expect(next.markers).toBe(DEFAULT_INK_PRESETS.markers);
    expect(patchPreset(next, 'freedraw', { stroke: '#00ff00' })).toBe(next);
  });

  test('a pen never becomes a marker, nor a marker a pen', () => {
    expect(patchPreset(DEFAULT_INK_PRESETS, 'freedraw', { pen: 'flatMarker' })).toBe(DEFAULT_INK_PRESETS);
    expect(patchPreset(DEFAULT_INK_PRESETS, 'highlighter', { pen: 'fountain' })).toBe(DEFAULT_INK_PRESETS);
    expect(activePreset(patchPreset(DEFAULT_INK_PRESETS, 'highlighter', { pen: 'flatMarker' }), 'highlighter').pen).toBe('flatMarker');
  });

  test('saved presets come back, and a damaged save falls back to the defaults', () => {
    withStorage();
    const changed = patchPreset(selectPreset(DEFAULT_INK_PRESETS, 'highlighter', 1), 'highlighter', { strokeWidth: 30 });
    saveInkPresets(changed);
    expect(loadInkPresets()).toEqual(changed);
    withStorage('{"pens":[{"pen":"roundMarker","stroke":"#000","strokeWidth":2,"opacity":1}],"pen":7,"markers":"no"}');
    expect(loadInkPresets()).toEqual(DEFAULT_INK_PRESETS);
    withStorage('not json');
    expect(loadInkPresets()).toEqual(DEFAULT_INK_PRESETS);
  });
});
