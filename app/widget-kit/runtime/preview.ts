/**
 * Timers scoped to one preview (docs/widgets.md#a-widget-draws-while-it-streams).
 * A preview's module shadows `setTimeout`, `setInterval`,
 * `requestAnimationFrame` and their cancels with these (`moduleCode` in the
 * parent), so a half-written effect that never cancels its interval can't
 * outlive its preview: the next program retires every older scope. A scope
 * also stops once its callbacks have run `PREVIEW_WORK_MS` in all, so a
 * preview left showing at a long pause can't spin. Kit and React code keep
 * the real timers.
 */

/** Callback time one preview may spend, in ms. */
export const PREVIEW_WORK_MS = 1000;

type Kind = "timeout" | "interval" | "frame";

interface Scope {
  handles: Map<number, Kind>;
  spent: number;
  stopped: boolean;
}

// Looked up per call: the globals are never replaced, only shadowed in a
// preview's module.
const real = {
  setTimeout: (fn: () => void, ms?: number) => window.setTimeout(fn, ms),
  clearTimeout: (handle: number) => window.clearTimeout(handle),
  setInterval: (fn: () => void, ms?: number) => window.setInterval(fn, ms),
  clearInterval: (handle: number) => window.clearInterval(handle),
  requestAnimationFrame: (fn: FrameRequestCallback) => window.requestAnimationFrame(fn),
  cancelAnimationFrame: (handle: number) => window.cancelAnimationFrame(handle),
};

const scopes = new Map<number, Scope>();

function cancel(handle: number, kind: Kind) {
  if (kind === "timeout") real.clearTimeout(handle);
  else if (kind === "interval") real.clearInterval(handle);
  else real.cancelAnimationFrame(handle);
}

function stop(scope: Scope) {
  scope.stopped = true;
  for (const [handle, kind] of scope.handles) cancel(handle, kind);
  scope.handles.clear();
}

/** Runs `fn` on the scope's budget; past it, the scope stops. */
function spend(scope: Scope, fn: () => void) {
  if (scope.stopped) return;
  const start = performance.now();
  try {
    fn();
  } finally {
    scope.spent += performance.now() - start;
    if (scope.spent > PREVIEW_WORK_MS) stop(scope);
  }
}

type Callback = (...args: unknown[]) => void;

/** The timer functions preview `id`'s module uses in place of the globals. */
export function previewScope(id: number) {
  const scope: Scope = { handles: new Map(), spent: 0, stopped: false };
  scopes.set(id, scope);
  const clear = (kind: Kind) => (handle?: number) => {
    if (handle == null) return;
    // Handles are per kind: a timeout and a frame may share a number.
    if (scope.handles.get(handle) === kind) scope.handles.delete(handle);
    cancel(handle, kind);
  };
  return {
    setTimeout(fn: unknown, ms?: number, ...args: unknown[]): number {
      if (typeof fn !== "function" || scope.stopped) return 0;
      const handle = real.setTimeout(() => {
        scope.handles.delete(handle);
        spend(scope, () => (fn as Callback)(...args));
      }, ms);
      scope.handles.set(handle, "timeout");
      return handle;
    },
    setInterval(fn: unknown, ms?: number, ...args: unknown[]): number {
      if (typeof fn !== "function" || scope.stopped) return 0;
      const handle = real.setInterval(() => spend(scope, () => (fn as Callback)(...args)), ms);
      scope.handles.set(handle, "interval");
      return handle;
    },
    requestAnimationFrame(fn: FrameRequestCallback): number {
      if (typeof fn !== "function" || scope.stopped) return 0;
      const handle = real.requestAnimationFrame((time) => {
        scope.handles.delete(handle);
        spend(scope, () => fn(time));
      });
      scope.handles.set(handle, "frame");
      return handle;
    },
    clearTimeout: clear("timeout"),
    clearInterval: clear("interval"),
    cancelAnimationFrame: clear("frame"),
  };
}

/** Stops every scope older than `id`: a newer program replaced them. */
export function retire(id: number) {
  for (const [key, scope] of scopes) {
    if (key >= id) continue;
    stop(scope);
    scopes.delete(key);
  }
}

/** Live handles in scope `id`; for tests. */
export const scopeHandles = (id: number) => scopes.get(id)?.handles.size ?? 0;
