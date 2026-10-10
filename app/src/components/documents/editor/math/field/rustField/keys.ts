import { redo, undo } from "@codemirror/commands";

import { newlineBeside } from "../fieldNote";
import { fieldKeys } from "../mathField/registry";
import { setMathMode } from "../mathField/visual-state";
import type { RustFieldController } from "./controller";

/**
 * The keys the note takes in the Rust field before its edit model does
 * (after the open list's, `popoverKey`): the toolbox's first (`fieldKeys`),
 * then Enter in a block, ⌘⇧M to TeX and the note's undo and redo.
 * Everything else, Esc, Shift+Enter, Tab and Backspace included, is the
 * model's. A `\command` being typed keeps undo off, as it isn't in the note
 * yet.
 */
export function hostKey(ctl: RustFieldController, e: KeyboardEvent): boolean {
  const { view } = ctl;
  if (view.state.facet(fieldKeys).some((take) => take(view, e, ctl))) return true;
  const side = enterSide(e, ctl.block, ctl.mode(), ctl.mv.field);
  if (side) {
    newlineBeside(view, ctl.target(), side === "before");
    return true;
  }
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
  return false;
}

/**
 * Where plain Enter in a field makes a line outside its maths: in a block,
 * after it, or before it with the caret at the field's very start; null
 * for the model's Enter — Shift+Enter (a block's new row), inline maths
 * (leaving it) and a `\command` being typed (committing it).
 */
export function enterSide(
  e: Pick<KeyboardEvent, "key" | "shiftKey" | "metaKey" | "ctrlKey" | "altKey">,
  block: boolean,
  mode: "math" | "text" | "command",
  field: { anchor: number; head: number },
): "before" | "after" | null {
  if (e.key !== "Enter" || e.shiftKey || e.metaKey || e.ctrlKey || e.altKey) return null;
  if (!block || mode === "command") return null;
  return field.anchor === 0 && field.head === 0 ? "before" : "after";
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
