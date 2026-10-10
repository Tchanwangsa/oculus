import { redo, undo } from "@codemirror/commands";

import { fieldKeys } from "../mathField/registry";
import { setMathMode } from "../mathField/visual-state";
import type { RustFieldController } from "./controller";

/**
 * The keys the note takes in the Rust field before its edit model does:
 * the toolbox's first (`fieldKeys`), then ⌘⇧M to TeX and the note's undo
 * and redo. Everything else, Esc, Enter, Tab and Backspace included, is
 * the model's. A `\command` being typed keeps undo off, as it isn't in the
 * note yet.
 */
export function hostKey(ctl: RustFieldController, e: KeyboardEvent): boolean {
  const { view } = ctl;
  if (view.state.facet(fieldKeys).some((take) => take(view, e, ctl))) return true;
  const mod = e.metaKey || e.ctrlKey;
  if (mod && e.shiftKey && !e.altKey && e.code === "KeyM") {
    if (ctl.target()) setMathMode(view, "tex");
    return true;
  }
  const key = e.key.toLowerCase();
  if (mod && !e.altKey && (key === "z" || (key === "y" && !e.shiftKey))) {
    history(ctl, e.shiftKey || key === "y");
    return true;
  }
  ctl.pendingAtKey = ctl.mv.field.pending;
  return false;
}

/** The note's history steps; the widget then reloads the field from the
 *  note (`sync`), or it closes when the step moved the caret out. */
function history(ctl: RustFieldController, redoing: boolean) {
  if (ctl.mode() === "command") return;
  (redoing ? redo : undo)(ctl.view);
}

/** Undo and redo from the Edit menu, as `beforeinput`. */
export function historyInput(ctl: RustFieldController, e: InputEvent) {
  if (e.inputType !== "historyUndo" && e.inputType !== "historyRedo") return;
  e.preventDefault();
  e.stopPropagation();
  history(ctl, e.inputType === "historyRedo");
}
