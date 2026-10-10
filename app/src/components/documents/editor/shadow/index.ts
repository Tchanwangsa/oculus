/**
 * Shadow mode (docs/editor-core.md#shadow-mode-checks-the-rust-core-against-every-note-editor):
 * in dev builds the Rust editor core, compiled to WebAssembly, mirrors every
 * transaction of every note editor and reports the first disagreement in
 * the console. A release build compiles none of it in. The wasm comes from
 * `bun run editor-wasm` into `pkg/` (gitignored); without it, or with
 * `localStorage["oculus.editorShadow"] = "off"`, the extension stays inert.
 * Only this file uses `import.meta`; the rest runs headless in bun.
 */
import type { Extension } from "@codemirror/state";

import { createEditorShadow, type EditorShadow } from "./core";
import type { ShadowWasm, WasmShadow } from "./types";

/** The generated glue's surface (`pkg/oculus_editor_core_wasm.d.ts`). */
interface Glue {
  default(): Promise<unknown>;
  Shadow: {
    seed(doc: string, selection: string, history: string | null): WasmShadow;
    nodeNames(): string[];
  };
}

const BUILD = "`bun run editor-wasm` in app/ builds it";

let shadow: EditorShadow | null = null;
let wasm: ShadowWasm | null = null;

function turnedOff(): boolean {
  try {
    return localStorage.getItem("oculus.editorShadow") === "off";
  } catch {
    return false;
  }
}

async function load(): Promise<ShadowWasm | null> {
  if (turnedOff()) {
    console.info('[editor-shadow] off: localStorage "oculus.editorShadow" is "off"');
    return null;
  }
  const importer = Object.values(import.meta.glob("./pkg/oculus_editor_core_wasm.js"))[0];
  if (!importer) {
    console.info(`[editor-shadow] off: the editor core's wasm is not built — ${BUILD}`);
    return null;
  }
  try {
    const glue = (await importer()) as Glue;
    await glue.default();
    const loaded: ShadowWasm = {
      seed: (doc, selection, history) => glue.Shadow.seed(doc, selection, history),
      nodeNames: glue.Shadow.nodeNames(),
    };
    console.info(
      '[editor-shadow] on: the Rust editor core mirrors every note editor; a mismatch logs "[editor-shadow] … mismatch"',
    );
    return loaded;
  } catch (e) {
    console.info(`[editor-shadow] off: the editor core's wasm did not load — ${BUILD}`, e);
    return null;
  }
}

/** The shadow for `noteExtensions()`: the same field and plugin for every
 *  editor in dev, nothing in a release build. */
export function editorShadow(): Extension {
  if (!import.meta.env.DEV) return [];
  if (!shadow) {
    shadow = createEditorShadow({ wasm: () => wasm });
    void load().then((w) => {
      wasm = w;
    });
  }
  return shadow.extension;
}
