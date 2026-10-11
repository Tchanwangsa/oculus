/** One editor state as the Rust editor core holds it: a `Shadow` from its
 *  WebAssembly bridge (`app/editor-core/wasm/src/lib.rs`). A value: every
 *  step returns a new one and leaves this one as it was. Typed here because
 *  the generated `pkg/` is gitignored and may not exist. Positions are UTF-16
 *  units; refusals throw. */
export interface WasmShadow {
  apply(
    changes: string,
    selection: string | null,
    userEvent: string | null,
    addToHistory: boolean,
    isolate: string | null,
    time: number,
  ): WasmShadow;
  /** `undefined` when the history has nothing to pop. */
  undo(time: number): WasmShadow | undefined;
  redo(time: number): WasmShadow | undefined;
  undoSelection(time: number): WasmShadow | undefined;
  redoSelection(time: number): WasmShadow | undefined;
  /** `fromA, toA, fromB, toB` per range of the step that made this one. */
  changedRanges(): Uint32Array;
  /** `changes.toJSON()` of that step, as a string; `undefined` for a seed. */
  changesJson(): string | undefined;
  historyJson(): string;
  length(): number;
  text(): string;
  slice(from: number, to: number): string;
  /** `selection.toJSON()`, as a string. */
  selectionJson(): string;
  undoDepth(): number;
  redoDepth(): number;
  /** `type id, from, to` per node in pre-order, `Document` first. */
  tree(): Uint32Array;
}

/** The loaded bridge. */
export interface ShadowWasm {
  /** `selection` is `EditorSelection` JSON whose ranges may carry
   *  `goalColumn`, `bidiLevel`, `assoc`, `from` and `to`; `history` is
   *  `historyField` JSON in the same rich form, plus `prevTime` and
   *  `prevUserEvent`. */
  seed(doc: string, selection: string, history: string | null): WasmShadow;
  /** Lezer's node names, indexed by the type ids `tree()` gives. */
  nodeNames: readonly string[];
}

/** One mirrored transaction as the `Shadow` calls that replay it. */
export type Step =
  | {
      op: "apply";
      changes: string;
      selection: string | null;
      userEvent: string | null;
      addToHistory: boolean;
      isolate: string | null;
      time: number;
    }
  | { op: Pop; time: number };

export type Pop = "undo" | "redo" | "undoSelection" | "redoSelection";

/** `seed`'s arguments, then each step in order, rebuild the shadow a report
 *  was made on (`runReplay`). */
export interface Replay {
  seed: { doc: string; selection: string; history: string | null };
  steps: Step[];
}

export type MismatchKind = "length" | "text" | "selection" | "undo" | "depth" | "doc" | "history" | "tree";

export interface Reporter {
  mismatch(kind: MismatchKind, details: Record<string, unknown>): void;
  error(e: unknown): void;
}
