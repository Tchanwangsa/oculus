import { Facet } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import type { FieldController } from "./controller/field-controller";

/** The one field per editor, for the toolbox. */
export const fields = new WeakMap<EditorView, FieldController>();

/** Keys the toolbox (`tools/mathTools`) takes in the field ahead of the field's
 *  own: Space for its quick picks, its shortcut, keys while it is open. A
 *  handler returns true when it took the key. */
export const fieldKeys = Facet.define<(view: EditorView, e: KeyboardEvent, field: FieldController) => boolean>();

export function activeMathField(view: EditorView): FieldController | null {
  return fields.get(view) ?? null;
}
