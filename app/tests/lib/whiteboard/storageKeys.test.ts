import { afterEach, beforeEach, expect, test } from 'bun:test';
import { adoptLegacyKey } from '../../../src/lib/whiteboard/storageKeys';

// Each test uses its own key: a key is adopted once per session, and other test files share the module.

let store: Map<string, string>;
const previous = (globalThis as { localStorage?: Storage }).localStorage;
beforeEach(() => {
  store = new Map();
  Object.assign(globalThis, { localStorage: {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => { store.set(key, value); },
    removeItem: (key: string) => { store.delete(key); },
  } });
});
afterEach(() => { Object.assign(globalThis, { localStorage: previous }); });

test('an entry under the canvas name moves to the whiteboard key', () => {
  store.set('oculus-canvas-cameras-test', '{"a":1}');
  adoptLegacyKey('oculus-whiteboard-cameras-test');
  expect(store.get('oculus-whiteboard-cameras-test')).toBe('{"a":1}');
  expect(store.has('oculus-canvas-cameras-test')).toBe(false);
});

test('a draft key moves the same way', () => {
  store.set('oculus:canvas-draft:999', 'draft');
  adoptLegacyKey('oculus:whiteboard-draft:999');
  expect(store.get('oculus:whiteboard-draft:999')).toBe('draft');
  expect(store.has('oculus:canvas-draft:999')).toBe(false);
});

test('an entry already under the whiteboard key wins and the old one is left alone', () => {
  store.set('oculus-whiteboard-tool-state-test', 'new');
  store.set('oculus-canvas-tool-state-test', 'old');
  adoptLegacyKey('oculus-whiteboard-tool-state-test');
  expect(store.get('oculus-whiteboard-tool-state-test')).toBe('new');
  expect(store.get('oculus-canvas-tool-state-test')).toBe('old');
});

test('nothing under either name writes nothing', () => {
  adoptLegacyKey('oculus-whiteboard-ink-presets-test');
  expect(store.size).toBe(0);
});

test('unavailable storage is ignored', () => {
  Object.assign(globalThis, { localStorage: { getItem: () => { throw new Error('denied'); } } });
  expect(() => adoptLegacyKey('oculus-whiteboard-tool-preferences-test')).not.toThrow();
});
