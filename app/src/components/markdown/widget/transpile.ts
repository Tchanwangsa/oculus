import { invoke } from "@tauri-apps/api/core";

export type Transpiled = { js: string; error?: undefined } | { js?: undefined; error: string };

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
  const run = invoke<{ js: string }>("widget_transpile", { source }).then(
    ({ js }): Transpiled => ({ js }),
    (e): Transpiled => ({ error: typeof e === "string" ? e : e instanceof Error ? e.message : String(e) }),
  );
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
