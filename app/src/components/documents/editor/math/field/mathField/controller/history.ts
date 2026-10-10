import { redo, undo } from "@codemirror/commands";

import type { FieldController } from "./field-controller";

/** Undo and redo are the note's: pending keystrokes go in first, then the
 *  note's history steps and the field reloads from it (`sync`), or closes
 *  when the step moved the caret out of this maths. */
export function history(ctl: FieldController, redoing: boolean) {
  // A command being typed (`\lam…`) isn't in the note yet.
  if (ctl.mf.mode === "latex") return;
  ctl.flush();
  (redoing ? redo : undo)(ctl.view);
}

export function historyInput(ctl: FieldController, e: InputEvent) {
  if (e.inputType !== "historyUndo" && e.inputType !== "historyRedo") return;
  e.preventDefault();
  e.stopPropagation();
  history(ctl, e.inputType === "historyRedo");
}
