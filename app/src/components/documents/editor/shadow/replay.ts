import type { Replay, ShadowWasm, WasmShadow } from "./types";

/** Rebuilds the shadow a report's `replay` describes: `seed`, then every
 *  step. Throws when a pop finds nothing, as the mirror reported. */
export function runReplay(wasm: ShadowWasm, replay: Replay): WasmShadow {
  const { doc, selection, history } = replay.seed;
  let shadow = wasm.seed(doc, selection, history);
  for (const [i, step] of replay.steps.entries()) {
    if (step.op === "apply") {
      shadow = shadow.apply(step.changes, step.selection, step.userEvent, step.addToHistory, step.isolate, step.time);
      continue;
    }
    const next = shadow[step.op](step.time);
    if (!next) throw new Error(`step ${i}: ${step.op} found nothing to pop`);
    shadow = next;
  }
  return shadow;
}
