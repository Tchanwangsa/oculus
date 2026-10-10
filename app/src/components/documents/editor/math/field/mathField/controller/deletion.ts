import { modelOf, type MlAtom } from "../model";
import type { FieldController } from "./field-controller";

/** The atom whose script the caret sits in when that script is empty
 *  (`\cos^{}`), else null. */
export function emptyScript(ctl: FieldController): MlAtom | null {
  const at = modelOf(ctl.mf)?.at(ctl.mf.position);
  const branch = at?.parentBranch;
  if (at?.type !== "first" || !at.parent || (branch !== "superscript" && branch !== "subscript")) return null;
  return at.parent.hasEmptyBranch(branch) ? at.parent : null;
}

/** Backspace in an empty script drops it, the caret just after its atom.
 *  MathLive drops it too, but an atom left with no branches (`\cos`) gets
 *  the caret at -2, which MathLive counts from the field's end. */
export function dropEmptyScript(ctl: FieldController) {
  const owner = emptyScript(ctl);
  if (!owner) return;
  ctl.mf.executeCommand("deleteBackward");
  const at = modelOf(ctl.mf)?.offsetOf(owner) ?? -1;
  if (!owner.hasChildren && at >= 0) ctl.mf.position = at;
}

/** Mod-Backspace, as in the note: the caret's line goes up to the caret —
 *  from the start of its row (a cell, in an environment's rows) through
 *  the structure the caret is in. At a line's start it is Backspace. */
export function deleteLineBackward(ctl: FieldController) {
  const mf = ctl.mf;
  if (mf.selectionIsCollapsed && !mf.getValue("latex-without-placeholders")) {
    removeMaths(ctl);
    return;
  }
  const model = modelOf(mf);
  let atom = mf.selectionIsCollapsed ? model?.at(mf.position) : undefined;
  // An atom of the root, or of a cell of an array that is (`\displaylines`).
  const inLine = (a: MlAtom) => !a.parent?.parent || (a.parent.type === "array" && !a.parent.parent.parent);
  while (atom?.parent && !inLine(atom)) atom = atom.parent;
  let first = atom;
  while (first?.leftSibling) first = first.leftSibling;
  const start = first ? model!.offsetOf(first) : -1;
  const end = atom ? model!.offsetOf(atom) : -1;
  if (start >= 0 && end > start) mf.selection = { ranges: [[start, end]], direction: "backward" };
  mf.executeCommand("deleteBackward");
}

/** Backspace in an empty field takes the maths (a block's lines) away. */
export function removeMaths(ctl: FieldController) {
  const { view } = ctl;
  const target = ctl.target();
  if (!target) return;
  const { doc } = view.state;
  let from = target.start;
  let to = target.end;
  if (target.block) {
    from = doc.lineAt(target.start).from;
    to = doc.lineAt(target.end).to;
    if (to < doc.length) to++;
    else if (from > 0) from--;
  }
  view.dispatch({ changes: { from, to }, selection: { anchor: from }, userEvent: "delete" });
  view.focus();
}
