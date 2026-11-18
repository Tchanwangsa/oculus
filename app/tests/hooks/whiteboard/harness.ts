import { mock } from 'bun:test';
import * as React from 'react';
import { createCanvasDocument, type CanvasDocument } from '@/lib/canvas/model';
import type { CanvasKind } from '@/lib/canvas/paper';
import type { EraserMode, EraserSize } from '@/lib/canvas/erase';

// Drives the real hook's handlers without a DOM renderer. State and refs live in slots that persist between
// explicit `render` calls; layout effects run after each render. Passive effects (DOM sizing, menus, the wheel) run
// once, on the first render, only for a harness given a `size`; otherwise they never do.
// The mock delegates to React outside `render`, so other test files that render components are unaffected.
const real = { ...React };
let slots: unknown[] = [], cursor = 0, layoutEffects: (() => void)[] = [], passiveEffects: (() => void)[] | null = null, rendering = false;
const hooks = {
  useRef: (value: unknown) => rendering ? (slots[cursor++] ??= { current: value }) : real.useRef(value),
  useState: (value: unknown) => {
    if (!rendering) return real.useState(value);
    const index = cursor++;
    if (!(index in slots)) slots[index] = typeof value === 'function' ? (value as () => unknown)() : value;
    return [slots[index], (next: unknown) => { slots[index] = typeof next === 'function' ? (next as (current: unknown) => unknown)(slots[index]) : next; }];
  },
  useCallback: (callback: unknown, deps?: unknown[]) => rendering ? (cursor++, callback) : real.useCallback(callback as () => void, deps ?? []),
  useEffect: (callback: () => void, deps?: unknown[]) => rendering ? void (cursor++, passiveEffects?.push(callback)) : real.useEffect(callback, deps),
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
  /** With a `size`: sends a wheel event to the canvas's listener, in screen pixels. */
  wheel: (event: { deltaX?: number; deltaY?: number; ctrlKey?: boolean; clientX?: number; clientY?: number }) => void;
  /** With a `size`: lays the editor out again at `size`, as its resize observer would. */
  resize: (size: { width: number; height: number }) => void;
  /** Runs the pending animation frame callbacks and returns how many ran. */
  frame: () => number;
  options: { eraserAutoReturn: boolean; eraserMode?: EraserMode; eraserSize?: EraserSize; localViewport: boolean; kind?: CanvasKind; size?: { width: number; height: number }; onInsertImage?: () => void };
}

/** Runs `effect` with a stand-in `window` when the test runtime has none, so DOM-sizing code can read it. */
function withWindow(effect: () => void) {
  const missing = typeof globalThis.window === 'undefined';
  if (missing) Object.assign(globalThis, { window: { devicePixelRatio: 1 } });
  try { effect(); } finally { if (missing) delete (globalThis as { window?: unknown }).window; }
}

/** `initial` is the document the hook mounts with; a `size` lays it out (a notebook's first fit) as the editor would. */
export function createHarness(options: Partial<Harness['options']> = {}, initial: CanvasDocument = createCanvasDocument()): Harness {
  slots = [];
  const frames = new Map<number, FrameRequestCallback>();
  let nextFrame = 1;
  globalThis.requestAnimationFrame = callback => { frames.set(nextFrame, callback); return nextFrame++; };
  globalThis.cancelAnimationFrame = id => { frames.delete(id); };
  const captures = new Set<number>();
  const listeners = new Map<string, (event: unknown) => void>();
  let layout = { width: 0, height: 0 }, observed: (() => void) | null = null;
  const canvas = {
    width: 0, height: 0,
    getBoundingClientRect: () => ({ left: 0, top: 0, ...layout }),
    getContext: () => null,
    addEventListener: (type: string, listener: (event: unknown) => void) => { listeners.set(type, listener); },
    removeEventListener: () => {},
    setPointerCapture: (id: number) => { captures.add(id); },
    hasPointerCapture: (id: number) => captures.has(id),
    releasePointerCapture: (id: number) => { captures.delete(id); },
  };
  const root = { focus: () => {}, contains: () => false, addEventListener: () => {}, removeEventListener: () => {}, getBoundingClientRect: () => ({ left: 0, top: 0, ...layout }) };
  let document = initial, mounted = false;
  const harness: Harness = {
    editor: undefined as unknown as Editor, published: [], views: [], captures, canvas,
    options: { eraserAutoReturn: false, localViewport: false, ...options },
    render(next = harness.editor?.documentRef.current ?? document) {
      document = next; cursor = 0; layoutEffects = [];
      passiveEffects = harness.options.size && !mounted ? [] : null;
      rendering = true;
      try {
        harness.editor = useCanvasInteraction(document, published => { harness.published.push(published); }, {
          onViewportChange: harness.options.localViewport ? view => { harness.views.push(view); } : undefined,
          eraserAutoReturn: harness.options.eraserAutoReturn,
          eraserMode: harness.options.eraserMode,
          eraserSize: harness.options.eraserSize,
          kind: harness.options.kind,
          onInsertImage: harness.options.onInsertImage,
        });
      } finally { rendering = false; }
      harness.editor.canvas.current = canvas as unknown as HTMLCanvasElement;
      harness.editor.root.current = root as unknown as HTMLDivElement;
      layoutEffects.forEach(effect => effect());
      const effects = passiveEffects;
      passiveEffects = null; mounted = true;
      if (effects) {
        layout = harness.options.size!;
        const ResizeObserver = class { constructor(callback: () => void) { observed = callback; } observe() {} disconnect() {} };
        Object.assign(globalThis, { ResizeObserver });
        withWindow(() => effects.forEach(effect => effect()));
      }
      return harness.editor;
    },
    wheel(event) {
      listeners.get('wheel')?.({ deltaX: 0, deltaY: 0, ctrlKey: false, metaKey: false, clientX: layout.width / 2, clientY: layout.height / 2, preventDefault: () => {}, ...event });
    },
    resize(size) { layout = size; withWindow(() => observed?.()); },
    frame() {
      const pending = [...frames.values()];
      frames.clear();
      for (const callback of pending) callback(performance.now());
      return pending.length;
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
