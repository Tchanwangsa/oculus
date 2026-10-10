import type { EditorState, Extension } from "@codemirror/state";

import { checkState, type CheckOptions } from "./checks";
import { Context } from "./context";
import { shadowField } from "./field";
import { shadowPlugin } from "./plugin";
import type { Replay, Reporter, ShadowWasm } from "./types";
import { isLive, replayOf } from "./value";

export interface EditorShadow {
  /** The field and the view plugin, for an editor's extensions. */
  extension: Extension;
  /** The view plugin's checks, for a state that no view holds. */
  check(state: EditorState, opts?: CheckOptions): void;
  /** Whether `state` has a shadow: `absent` without the extension. */
  status(state: EditorState): "absent" | "unseeded" | "live" | "stopped";
  /** What a report on `state` would carry to rebuild its shadow. */
  replayOf(state: EditorState): Replay | null;
}

/** Shadow mode with no `import.meta`, so bun can drive it headless with the
 *  wasm it loaded itself. `wasm` gives null until the bridge is ready;
 *  states seed lazily once it is. Reports go to the console by default. */
export function createEditorShadow(opts: { wasm: () => ShadowWasm | null; reporter?: Reporter }): EditorShadow {
  const reporter: Reporter = opts.reporter ?? {
    mismatch: (kind, details) => console.error(`[editor-shadow] ${kind} mismatch`, details),
    error: (e) => console.error("[editor-shadow] error", e),
  };
  const ctx = new Context(opts.wasm, reporter);
  const field = shadowField(ctx);
  const check = (state: EditorState, checkOpts?: CheckOptions) => checkState(ctx, field, state, checkOpts);
  return {
    extension: [field, shadowPlugin(check)],
    check,
    status(state) {
      const value = state.field(field, false);
      if (value === undefined) return "absent";
      if (isLive(value)) return value.chain.stopped ? "stopped" : "live";
      return value;
    },
    replayOf(state) {
      const value = state.field(field, false);
      return isLive(value) ? replayOf(value) : null;
    },
  };
}
