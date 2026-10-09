import { modelOf } from "../model";
import type { FieldController } from "./field-controller";

/** The caret is on a line holding nothing: the whole block when empty, or
 *  one row of its lines (a root `lines` table, whose rows MathLive keeps
 *  as branches of one array atom). */
export function onEmptyLine(ctl: FieldController): boolean {
  const model = modelOf(ctl.mf);
  const at = ctl.mf.position;
  const here = model?.at(at);
  if (!model || !here || here.type !== "first") return false;
  if (here.parent?.type !== "root" && here.parent?.environmentName !== "lines") return false;
  // The row's own atoms, not the next offset's: that steps inside a
  // leading `\left(` and would read a full row as empty.
  return here.hasNoSiblings;
}

/** Show the hint on an empty line, hide it elsewhere. Two frames on, once
 *  MathLive has drawn the edit (it draws in a frame of its own), a block's
 *  goes at the height of the caret's line: its empty row's leading atom,
 *  not the hidden caret, which can still be where it was. */
export function syncHint(ctl: FieldController) {
  if (!ctl.hint || ctl.hintFrame) return;
  ctl.hintFrame = requestAnimationFrame(() => {
    ctl.hintFrame = requestAnimationFrame(() => {
      ctl.hintFrame = 0;
      placeHint(ctl);
    });
  });
}

function placeHint(ctl: FieldController) {
  const hint = ctl.hint;
  if (!hint) return;
  const live = !ctl.dead && ctl.dom.isConnected && ctl.mf.mode === "math" && onEmptyLine(ctl);
  const line = live && ctl.block ? ctl.mf.getElementInfo(ctl.mf.position)?.bounds : null;
  ctl.dom.classList.toggle("cm-math-field-empty", line != null);
  if (!live || (ctl.block && !line)) {
    hint.hidden = true;
    return;
  }
  hint.hidden = false;
  if (line) {
    // The row's centre, kept so the hint stays inside the field's box.
    const box = ctl.dom.getBoundingClientRect();
    const half = hint.offsetHeight / 2;
    const centre = line.top + line.height / 2 - box.top;
    hint.style.top = `${Math.min(Math.max(centre, half), box.height - half)}px`;
  }
}
