import { invoke } from "@tauri-apps/api/core";

/** `pending`: components the module draws that it never declares (a
 *  streaming prefix's not-yet-written parts); empty for a whole widget that
 *  declares everything it draws. */
export type Transpiled = { js: string; pending: string[]; error?: undefined } | { js?: undefined; error: string };

type Output = { js: string; pending?: string[] };

const call = (source: string): Promise<Transpiled> =>
  invoke<Output>("widget_transpile", { source }).then(
    ({ js, pending }): Transpiled => ({ js, pending: pending ?? [] }),
    (e): Transpiled => ({ error: typeof e === "string" ? e : e instanceof Error ? e.message : String(e) }),
  );

const pending = new Map<string, Promise<Transpiled>>();
const done = new Map<string, Transpiled>();
/** Sources kept; a thread's widgets fit many times over. */
const MAX = 64;

/**
 * The widget's TSX as an ES module, through `widget_transpile` (oxc in Rust,
 * the same transform as `oculus widget check`). Its errors are strings
 * (`"<line>:<col> <message>"`, or a refused import) and are kept like
 * results: the same source fails the same way. Cached by source, so a
 * remount (commit, theme, scrolling) never asks again.
 */
export function transpile(source: string): Promise<Transpiled> {
  const hit = pending.get(source);
  if (hit) return hit;
  const run = call(source);
  run.then((result) => {
    done.set(source, result);
    if (done.size > MAX) {
      const oldest = done.keys().next().value!;
      done.delete(oldest);
      pending.delete(oldest);
    }
  });
  pending.set(source, run);
  return run;
}

/** A finished transpile of `source`, if there is one. */
export const transpiled = (source: string) => done.get(source);

/** A streaming prefix through the same transpile, never cached: each is
 *  tried once and would push committed widgets out of the cache. */
export const transpilePreview = call;
