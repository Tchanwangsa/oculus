/**
 * The widget's state: one flat object of JSON values by key, seeded from the
 * `#oculus-state` block the parent writes into `srcdoc` and posted back whole
 * after each change, debounced, so the parent can keep it in `widgetStore`.
 * Shared by `oculus/state` and the runtime through one chunk.
 */
export type State = Record<string, unknown>;

/** Quiet time before a change is posted: a slider drag posts once it rests. */
export const POST_DEBOUNCE_MS = 200;

let state: State = {};
const listeners = new Set<() => void>();
let publish: ((state: State) => void) | null = null;
let pending: ReturnType<typeof setTimeout> | null = null;

export function getState(): State {
  return state;
}

export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function notify() {
  for (const listener of [...listeners]) listener();
}

/** Where changes go; the runtime sets it to post to the parent. */
export function onPublish(fn: ((state: State) => void) | null) {
  publish = fn;
}

/** Replaces the state without publishing it: the seed, or the parent's copy. */
export function replaceState(next: State) {
  state = { ...next };
  notify();
}

export function setValue(key: string, value: unknown) {
  if (Object.is(state[key], value)) return;
  state = { ...state, [key]: value };
  notify();
  if (pending != null) clearTimeout(pending);
  pending = setTimeout(() => {
    pending = null;
    publish?.(state);
  }, POST_DEBOUNCE_MS);
}
