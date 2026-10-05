import { StateEffect, StateField, type EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

/**
 * Whether Live mode treats the editor as focused, which decides what reveals
 * its source. A maths field (`mathField.ts`) holding focus counts: it is a
 * widget inside the note, and focus moving into it must not redraw the note
 * as if the editor had blurred.
 */

export const setFocused = StateEffect.define<boolean>();

export const focusedField = StateField.define<boolean>({
  create: () => false,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setFocused)) value = e.value;
    return value;
  },
});

/** The focused element is a maths field inside an editor. CodeMirror reports
 *  the change 10 ms after the event, so focus has settled by then. */
export function mathFieldFocused(): boolean {
  const active = document.activeElement;
  return active instanceof HTMLElement && active.tagName === "MATH-FIELD" && active.closest(".cm-editor") != null;
}

export const trackFocus = EditorView.focusChangeEffect.of((_state, focusing) =>
  setFocused.of(focusing || mathFieldFocused()),
);

export function liveFocused(state: EditorState): boolean {
  return state.field(focusedField, false) ?? false;
}

/** Tell a freshly configured Live mode whether the editor has focus: the
 *  field starts unfocused, and only a focus change would correct it. */
export function syncLiveFocus(view: EditorView): void {
  if (view.state.field(focusedField, false) !== undefined) {
    view.dispatch({ effects: setFocused.of(view.hasFocus || mathFieldFocused()) });
  }
}
