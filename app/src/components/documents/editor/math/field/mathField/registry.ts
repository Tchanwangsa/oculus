import { Facet } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import type { Direction } from "../fieldNote";
import type { Box } from "./geometry";

/**
 * The open visual field, whichever engine draws it: `FieldController`
 * (MathLive) or `RustFieldController` (`math/field/rustField`). The widget,
 * the toolbox and the note's undo use only this, never an engine's own
 * element.
 */
export interface VisualField {
  readonly dom: HTMLElement;
  readonly view: EditorView;
  readonly display: boolean;
  /** On lines of its own (the block layer's), else in a line. */
  readonly block: boolean;
  /** The maths this field edits (`ActiveMath.id`), for its whole life. */
  readonly id: number;
  /** What typing does at the caret: maths, text (`\text{}`), or a
   *  `\command` being typed, whose keys are its own. */
  mode(): "math" | "text" | "command";
  /** Calls `listener` when the field's caret or selection moves from where
   *  it is now; returns the unsubscribe. */
  onSelectionChange(listener: () => void): () => void;
  /** Nothing typed in the field (a bare `$$` inline pair, a fresh block). */
  isEmpty(): boolean;
  /** Space is free for the toolbox's quick picks here. */
  spaceFree(): boolean;
  /** The caret's viewport rect, for the quick picks to hang from. */
  caretRect(): Box | null;
  /** A palette entry: `#{}`/`${}` slots become empty slots, the first
   *  taking the selection. */
  insertTemplate(template: string): void;
  /** Keystrokes not yet in the note go in (`isolate`: as an undo step of
   *  their own). A no-op for a field that writes as it goes. */
  flush(isolate?: boolean): void;
  leave(dir: Direction): void;
  /** The note's LaTeX changed under the field (an undo): show it. */
  sync(source: string): void;
  destroy(): void;
}

/** The one field per editor, for the toolbox. */
export const fields = new WeakMap<EditorView, VisualField>();

/** Keys the toolbox (`tools/mathTools`) takes in the field ahead of the field's
 *  own: Space for its quick picks, its shortcut, keys while it is open. A
 *  handler returns true when it took the key. */
export const fieldKeys = Facet.define<(view: EditorView, e: KeyboardEvent, field: VisualField) => boolean>();

export function activeMathField(view: EditorView): VisualField | null {
  return fields.get(view) ?? null;
}
