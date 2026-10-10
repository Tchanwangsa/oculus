import type { MathView } from "./index";

/** Px from the frame's visible edge a sideways-scrolled block keeps the
 *  caret. */
const SCROLL_MARGIN = 8;

/** The caret at the selection's head, hidden while a selection is drawn or
 *  nothing measured it; the textarea and an IME's text go with it, so the
 *  candidate window opens there. A block scrolls sideways to keep it. */
export function drawCaret(view: MathView) {
  const { field, measured, caret, input, preedit, frame } = view;
  const id = field.head;
  const x = measured.x[id];
  const measuredHere = Number.isFinite(x);
  const [from, to] = field.selected;
  caret.hidden = !measuredHere || from !== to;
  if (!measuredHere) return;
  const top = measured.top[id];
  const height = Math.max(measured.bottom[id] - top, 1);
  for (const el of [caret, input, preedit]) {
    el.style.left = `${x}px`;
    el.style.top = `${top}px`;
  }
  caret.style.height = input.style.height = `${height}px`;
  preedit.style.height = `${height}px`;
  preedit.style.lineHeight = `${height}px`;
  restartBlink(view);
  if (view.block) {
    const visible = frame.clientWidth;
    if (x < frame.scrollLeft + SCROLL_MARGIN) frame.scrollLeft = Math.max(0, x - SCROLL_MARGIN);
    else if (x > frame.scrollLeft + visible - SCROLL_MARGIN) frame.scrollLeft = x - visible + SCROLL_MARGIN;
  }
}

/** The caret shows solid again after each move, then blinks. */
export function restartBlink(view: MathView) {
  const { caret } = view;
  caret.classList.remove("cm-math-view-blink");
  void caret.offsetWidth;
  caret.classList.add("cm-math-view-blink");
}
