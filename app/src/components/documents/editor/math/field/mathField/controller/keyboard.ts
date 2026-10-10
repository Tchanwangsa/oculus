import { fieldKeys } from "../registry";
import { WHOLE_ENV } from "../serialize";
import { setMathMode } from "../visual-state";
import { atTextEnd } from "./command-mode";
import { deleteLineBackward, dropEmptyScript, emptyScript, removeMaths } from "./deletion";
import { editGrid, gridStep } from "./grid";
import { onEmptyLine } from "./hint";
import { history } from "./history";
import type { Direction, FieldController } from "./field-controller";

/** Our keys: the toolbox's first (`fieldKeys`), then the matrix keys, leave,
 *  new line, Tab between slots, toggle to TeX, delete the maths when empty,
 *  the note's undo and redo. Commands being typed (`\lam…`) keep MathLive's. */
export function onKey(ctl: FieldController, e: KeyboardEvent) {
  if (e.isComposing) return;
  const mf = ctl.mf;
  const typingCommand = mf.mode === "latex";
  const mod = e.metaKey || e.ctrlKey;
  const stop = () => {
    e.preventDefault();
    e.stopPropagation();
  };
  if (ctl.view.state.facet(fieldKeys).some((take) => take(ctl.view, e, ctl))) {
    stop();
    return;
  }
  // Space, `;` and Backspace unshifted; the closing brackets are shifted keys.
  const plain = !mod && !e.altKey && (!e.shiftKey || !/^(?: |;|Backspace)$/.test(e.key));
  const grid = plain ? gridStep(ctl, e.key) : null;
  if (grid) {
    stop();
    editGrid(ctl, grid, e.key === ";");
  } else if (e.key === "Escape" && !typingCommand) {
    stop();
    ctl.leave("forward");
  } else if (e.key === "Enter" && !typingCommand && !mod && !e.altKey) {
    stop();
    // Never a second empty line: Enter on an empty one does nothing.
    if (!ctl.display) ctl.leave("forward");
    else if (ctl.mf.mode !== "math" || !onEmptyLine(ctl)) newLine(ctl);
  } else if (
    mf.mode === "text" &&
    !mod &&
    !e.altKey &&
    !e.shiftKey &&
    (e.key === "Tab" || (e.key === "ArrowRight" && mf.selectionIsCollapsed && atTextEnd(ctl)))
  ) {
    // Out of the text, as → leaves a fraction's slot: maths again.
    stop();
    while (!atTextEnd(ctl)) mf.executeCommand("moveToNextChar");
    mf.executeCommand(["switchMode", "math"]);
    mf.applyStyle({ fontSeries: "auto", fontShape: "auto" });
  } else if (e.key === "Tab" && !typingCommand && !mod && !e.altKey) {
    stop();
    tab(ctl, e.shiftKey);
  } else if (e.key === "Backspace" && mod && !e.altKey && !e.shiftKey) {
    stop();
    deleteLineBackward(ctl);
  } else if (e.key === "Backspace" && !mod && mf.selectionIsCollapsed && !mf.getValue("latex-without-placeholders")) {
    stop();
    removeMaths(ctl);
  } else if (e.key === "Backspace" && !mod && mf.selectionIsCollapsed && emptyScript(ctl)) {
    stop();
    dropEmptyScript(ctl);
  } else if (mod && e.shiftKey && !e.altKey && e.code === "KeyM") {
    stop();
    ctl.flush();
    if (ctl.target()) setMathMode(ctl.view, "tex");
  } else if (mod && !e.altKey && (e.key.toLowerCase() === "z" || (e.key.toLowerCase() === "y" && !e.shiftKey))) {
    stop();
    history(ctl, e.shiftKey || e.key.toLowerCase() === "y");
  }
}

/** A row after the caret's (MathLive splits it in a multi-line
 *  environment); a caret just outside a whole-value environment moves in
 *  first, so the row joins it rather than wrapping it. */
function newLine(ctl: FieldController) {
  const mf = ctl.mf;
  if (WHOLE_ENV.test(mf.getValue())) {
    if (mf.position === mf.lastOffset) mf.position = mf.lastOffset - 1;
    else if (mf.position === 0) mf.position = 1;
  }
  mf.executeCommand("addRowAfter");
}

/** Tab: the next empty slot, else a `\qquad`. Shift-Tab: the slot before,
 *  else nothing. */
function tab(ctl: FieldController, back: boolean) {
  ctl.tabbing = true;
  ctl.tabFailed = false;
  ctl.mf.executeCommand(back ? "moveToPreviousPlaceholder" : "moveToNextPlaceholder");
  ctl.tabbing = false;
  if (ctl.tabFailed && !back) ctl.mf.insert("\\qquad", { format: "latex" });
}

export function moveOut(ctl: FieldController, e: CustomEvent<{ direction: Direction }>) {
  e.preventDefault();
  if (ctl.tabbing) {
    ctl.tabFailed = true;
    return;
  }
  // MathLive announces the move after this returns; leaving now would
  // unmount the field and dispose its model under that code.
  const dir = e.detail.direction;
  queueMicrotask(() => {
    if (ctl.dom.isConnected) ctl.leave(dir);
  });
}
