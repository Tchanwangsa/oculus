import { applyGridEdit, gridKey, type GridStep } from "../../mathMatrixField";
import { modelOf } from "../model";
import { syncHint } from "./hint";
import type { FieldController } from "./field-controller";

/** What a key does in the matrix or bracket group at the caret
 *  (`mathMatrixField.ts`), or null when it does nothing special there. */
export function gridStep(ctl: FieldController, key: string): GridStep | null {
  const model = modelOf(ctl.mf);
  return model ? gridKey(ctl.mf, model, key) : null;
}

/** A matrix key's edit, an undo step of its own: pending keystrokes go in
 *  first. A `;` turning a bracket group into a matrix is first typed as a
 *  step of its own, so ⌘Z gives back `f(x;` for maths that meant it. */
export function editGrid(ctl: FieldController, step: GridStep, semicolon: boolean) {
  const model = modelOf(ctl.mf);
  if (!model) return;
  ctl.flush();
  if (semicolon && step.at.group) {
    ctl.mf.insert(";", { format: "latex", mode: "math" });
    ctl.flush(true);
  }
  applyGridEdit(ctl.mf, model, step.at, step.edit);
  ctl.flush(true);
  syncHint(ctl);
}
