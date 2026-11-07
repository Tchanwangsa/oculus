/**
 * Loads the maths engine chunk once per frame, for every part of the kit that
 * typesets (`Tex` in `oculus/ui`, labels in `oculus/2d`). A failure is posted
 * as a kit error, once.
 */
import { describe, post } from "../runtime/post";

export type TexEngine = typeof import("./engine");

let engine: TexEngine | null = null;
let loading: Promise<void> | null = null;
const listeners = new Set<() => void>();

export function loadTex(): Promise<void> {
  loading ??= import("./engine")
    .then(async (mod) => {
      await mod.loadEngine();
      engine = mod;
      for (const listener of [...listeners]) listener();
    })
    .catch((error) => post({ kind: "error", phase: "kit", message: `Maths did not load: ${describe(error)}` }));
  return loading;
}

/** The engine once it is in, else null. */
export const texEngine = () => engine;

/** Calls `listener` when the engine comes in; returns the unsubscribe. */
export function onTexReady(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
