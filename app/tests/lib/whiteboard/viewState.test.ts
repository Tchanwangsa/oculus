import { afterEach, beforeEach, expect, test } from 'bun:test';
import { DEFAULT_DRAWING_STYLE } from '@/lib/whiteboard/style';
import { loadCamera, loadToolState, saveCamera, saveToolState } from '@/lib/whiteboard/viewState';

const previous = (globalThis as { localStorage?: Storage }).localStorage;
let store: Map<string, string>;
beforeEach(() => {
  store = new Map();
  Object.assign(globalThis, { localStorage: { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => { store.set(key, value); } } });
});
afterEach(() => { Object.assign(globalThis, { localStorage: previous }); });

test('the tool and style come back as saved', () => {
  saveToolState({ tool: 'freedraw', style: { ...DEFAULT_DRAWING_STYLE, stroke: '#e03131', strokeWidth: 4 } });
  const state = loadToolState();
  expect(state.tool).toBe('freedraw');
  expect(state.style.stroke).toBe('#e03131');
  expect(state.style.strokeWidth).toBe(4);
});

test('crop, unknown tools and damaged fields fall back without losing the rest', () => {
  store.set('oculus-whiteboard-tool-state-v1', JSON.stringify({ tool: 'crop', style: { stroke: 7, strokeWidth: 4, opacity: null } }));
  const state = loadToolState();
  expect(state.tool).toBe('select');
  expect(state.style.stroke).toBe(DEFAULT_DRAWING_STYLE.stroke);
  expect(state.style.strokeWidth).toBe(4);
  expect(state.style.opacity).toBe(DEFAULT_DRAWING_STYLE.opacity);
  store.set('oculus-whiteboard-tool-state-v1', 'not json');
  expect(loadToolState()).toEqual({ tool: 'select', style: DEFAULT_DRAWING_STYLE });
});

test('each drawing keeps its own camera, and the oldest are forgotten past the limit', () => {
  saveCamera('file:a', { x: 10, y: -2000, zoom: 1.5 }, 1);
  saveCamera('file:b', { x: 0, y: 0, zoom: 1 }, 2);
  expect(loadCamera('file:a')).toEqual({ x: 10, y: -2000, zoom: 1.5 });
  expect(loadCamera('file:c')).toBeNull();
  for (let index = 0; index < 60; index++) saveCamera(`file:n${index}`, { x: index, y: 0, zoom: 1 }, 10 + index);
  expect(loadCamera('file:a')).toBeNull();
  expect(loadCamera('file:n59')).toEqual({ x: 59, y: 0, zoom: 1 });
});

test('no storage means a fresh start, not an error', () => {
  Object.assign(globalThis, { localStorage: undefined });
  expect(loadToolState().tool).toBe('select');
  expect(loadCamera('file:a')).toBeNull();
  expect(() => saveCamera('file:a', { x: 0, y: 0, zoom: 1 })).not.toThrow();
});
