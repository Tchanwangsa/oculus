/**
 * The maths engine's instances: the compiled wasm module (`math-core/wasm`,
 * the katex fork), the instance calls go to, and a spare.
 *
 * A trap (a panic, or a stack overflow from a few hundred levels of nesting)
 * leaves an instance unusable, and wasm-bindgen's glue caches its instance in
 * a module-level variable. So each instance gets its own copy of the glue,
 * imported from a blob URL of its source, and one copy is kept imported in
 * reserve: on a trap the spare is instantiated synchronously from the kept
 * module, the call fails as a render error, and another spare is imported.
 */
export type Glue = typeof import("../../../math-core/pkg/oculus_math.js");

let module: WebAssembly.Module | null = null;
let glueSource = "";
let current: Glue | null = null;
let spare: Glue | null = null;
let filling: Promise<void> | null = null;
const listeners = new Set<() => void>();

/** A formula that trapped, by its source: a retry with other options (the
 *  markdown plugin's lenient pass) or a re-render would trap again. */
const trapped = new Map<string, MathsTrap>();
const TRAPPED_MAX = 200;

/** A call the engine trapped on: a render error, never KaTeX's `ParseError`. */
export class MathsTrap extends Error {
  constructor(cause: Error) {
    super(`The maths engine failed on this formula (${cause.message})`, { cause });
    this.name = "MathsTrap";
  }
}

/** The engine is instantiated: renders run synchronously. */
export function mathsReady(): boolean {
  return current != null;
}

/** Calls `fn` after each change of `mathsReady()`, in a microtask, so a
 *  change during a render (a trap inside a CodeMirror update or a React
 *  render) never re-enters it. Returns the unsubscribe. */
export function onMathsReady(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function notify() {
  queueMicrotask(() => {
    for (const fn of [...listeners]) fn();
  });
}

async function importGlue(): Promise<Glue> {
  const url = URL.createObjectURL(new Blob([glueSource], { type: "text/javascript" }));
  try {
    return (await import(/* @vite-ignore */ url)) as Glue;
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Imports glue copies until an instance is up and a spare is in reserve. */
function refill(): Promise<void> {
  if (current && spare) return Promise.resolve();
  filling ??= (async () => {
    try {
      while (!current || !spare) {
        const glue = await importGlue();
        if (current) spare = glue;
        else {
          glue.initSync({ module: module! });
          current = glue;
          notify();
        }
      }
    } finally {
      // Cleared in the same task the loop ends in, so a trap after it starts a new fill.
      filling = null;
    }
  })();
  return filling;
}

/** Puts the engine up from a compiled module and the glue's source text
 *  (`load.ts`); resolves once it renders and a spare is in reserve. */
export function installMaths(compiled: WebAssembly.Module, source: string): Promise<void> {
  module = compiled;
  glueSource = source;
  current = spare = null;
  return refill();
}

function isTrap(e: unknown): e is Error {
  return e instanceof WebAssembly.RuntimeError || e instanceof RangeError;
}

/** Runs `fn` on the current instance. Throws before the engine is ready
 *  (callers check `mathsReady()`), and a `MathsTrap` when the instance traps,
 *  after swapping in the spare. */
export function callMaths<T>(tex: string, fn: (glue: Glue) => T): T {
  if (!current) throw new Error("The maths engine is not loaded yet");
  const known = trapped.get(tex);
  if (known) throw known;
  try {
    return fn(current);
  } catch (e) {
    if (!isTrap(e)) throw e;
    const error = new MathsTrap(e);
    if (trapped.size >= TRAPPED_MAX) trapped.clear();
    trapped.set(tex, error);
    current = null;
    if (spare) {
      spare.initSync({ module: module! });
      current = spare;
      spare = null;
    } else notify();
    void refill();
    throw error;
  }
}
