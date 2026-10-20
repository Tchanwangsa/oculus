import { mock } from 'bun:test';
import * as React from 'react';
import { createCanvasDocument, type CanvasDocument } from '@/lib/canvas/model';

// Drives the real hook's handlers without a DOM renderer. State and refs live in slots that persist between
// explicit `render` calls; layout effects run after each render, passive effects (DOM sizing, menus) never do.
// The mock delegates to React outside `render`, so other test files that render components are unaffected.
const real = { ...React };
let slots: unknown[] = [], cursor = 0, layoutEffects: (() => void)[] = [], rendering = false;
const hooks = {
  useRef: (value: unknown) => rendering ? (slots[cursor++] ??= { current: value }) : real.useRef(value),
  useState: (value: unknown) => {
    if (!rendering) return real.useState(value);
    const index = cursor++;
    if (!(index in slots)) slots[index] = typeof value === 'function' ? (value as () => unknown)() : value;
    return [slots[index], (next: unknown) => { slots[index] = typeof next === 'function' ? (next as (current: unknown) => unknown)(slots[index]) : next; }];
  },
  useCallback: (callback: unknown, deps?: unknown[]) => rendering ? (cursor++, callback) : real.useCallback(callback as () => void, deps ?? []),
  useEffect: (callback: () => void, deps?: unknown[]) => rendering ? void cursor++ : real.useEffect(callback, deps),
  useLayoutEffect: (callback: () => void, deps?: unknown[]) => rendering ? void (cursor++, layoutEffects.push(callback)) : real.useLayoutEffect(callback, deps),
};
mock.module('react', () => ({ ...real, ...hooks, default: { ...(real as { default?: object }).default, ...hooks } }));
const { useCanvasInteraction } = await import('@/hooks/canvas/useCanvasInteraction');

export type Editor = ReturnType<typeof useCanvasInteraction>;
type PointerInput = Parameters<Editor['pointerDown']>[0];
export interface Harness {
  editor: Editor;
  /** Every document handed to `onChange`, in order. */
  published: CanvasDocument[];
  /** Every viewport handed to `onViewportChange`, when the host keeps the camera. */
  views: CanvasDocument['viewport'][];
  /** Pointer captures held, by pointer id. */
  captures: Set<number>;
  canvas: { setPointerCapture: (id: number) => void; releasePointerCapture: (id: number) => void; hasPointerCapture: (id: number) => boolean };
  /** Renders with `next` as the host's document; by default the host echoes the hook's latest document. */
  render: (next?: CanvasDocument) => Editor;
  options: { eraserAutoReturn: boolean; localViewport: boolean };
}

export function createHarness(options: Partial<Harness['options']> = {}): Harness {
  slots = [];
  globalThis.requestAnimationFrame = (() => 1) as typeof requestAnimationFrame;
  const captures = new Set<number>();
  const canvas = {
    getBoundingClientRect: () => ({ left: 0, top: 0 }),
    getContext: () => null,
    setPointerCapture: (id: number) => { captures.add(id); },
    hasPointerCapture: (id: number) => captures.has(id),
    releasePointerCapture: (id: number) => { captures.delete(id); },
  };
  let document = createCanvasDocument();
  const harness: Harness = {
    editor: undefined as unknown as Editor, published: [], views: [], captures, canvas,
    options: { eraserAutoReturn: false, localViewport: false, ...options },
    render(next = harness.editor?.documentRef.current ?? document) {
      document = next; cursor = 0; layoutEffects = [];
      rendering = true;
      try {
        harness.editor = useCanvasInteraction(document, published => { harness.published.push(published); }, {
          onViewportChange: harness.options.localViewport ? view => { harness.views.push(view); } : undefined,
          eraserAutoReturn: harness.options.eraserAutoReturn,
        });
      } finally { rendering = false; }
      harness.editor.canvas.current = canvas as unknown as HTMLCanvasElement;
      harness.editor.root.current = { focus: () => {} } as unknown as HTMLDivElement;
      layoutEffects.forEach(effect => effect());
      return harness.editor;
    },
  };
  current = harness;
  harness.render(document);
  return harness;
}

export function pointer(id: number, type = 'pen', x = 10, y = 10, extra: Record<string, unknown> = {}, samples: { clientX: number; clientY: number }[] = []): PointerInput {
  return {
    pointerId: id, pointerType: type, button: 0, buttons: 0, clientX: x, clientY: y,
    shiftKey: false, metaKey: false, ctrlKey: false, preventDefault: () => {},
    currentTarget: harnessCanvas(), nativeEvent: { getCoalescedEvents: () => samples }, ...extra,
  } as unknown as PointerInput;
}

/** The latest harness; `pointer` events target its canvas. */
let current: Harness | undefined;
const harnessCanvas = () => current!.canvas;

export function key(name: string, modifiers: { meta?: boolean; shift?: boolean } = {}): Parameters<Editor['keyDown']>[0] {
  return {
    key: name, code: '', metaKey: !!modifiers.meta, ctrlKey: false, shiftKey: !!modifiers.shift, altKey: false,
    target: { closest: () => null }, preventDefault: () => {}, stopPropagation: () => {}, defaultPrevented: false,
  } as unknown as Parameters<Editor['keyDown']>[0];
}
