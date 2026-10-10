import type { RustFieldController } from "./controller";

/** The caret is alone on a top-level row holding nothing: an empty field,
 *  or the empty row after a block's trailing `\\`. */
function onEmptyLine(ctl: RustFieldController): boolean {
  const f = ctl.mv.field;
  if (f.mode !== "math" || f.anchor !== f.head) return false;
  const slot = f.slots[f.stopSlots[f.head]];
  return slot != null && slot.kind === "row" && slot.parent == null && slot.from === slot.to;
}

/**
 * The prompt to Space for the toolbox on an empty line: after an empty
 * inline field in flow; in a block centred on the caret's row, which draws
 * the caret just before it (the view's own is hidden). The view draws
 * synchronously, so the caret's box is current.
 */
export function syncHint(ctl: RustFieldController) {
  const { hint, mv, dom } = ctl;
  const live = !ctl.dead && !mv.dead && dom.isConnected && onEmptyLine(ctl);
  const caret = live && ctl.block ? mv.caretRect() : null;
  dom.classList.toggle("cm-math-view-empty", caret != null);
  if (!live || (ctl.block && !caret)) {
    hint.hidden = true;
    return;
  }
  hint.hidden = false;
  if (caret) {
    // The row's centre, kept so the hint stays inside the field's box.
    const box = dom.getBoundingClientRect();
    const half = hint.offsetHeight / 2;
    const centre = (caret.top + caret.bottom) / 2 - box.top;
    hint.style.top = `${Math.min(Math.max(centre, half), box.height - half)}px`;
  }
}
