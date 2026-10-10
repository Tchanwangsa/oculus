import type { MathView } from "./index";

/**
 * A press puts the caret at the stop it lands on (`stopAt`); with Shift it
 * extends the selection there. Dragging selects from the press to the
 * pointer, the model widening it over whole structures.
 */
export function pointerDown(view: MathView, e: PointerEvent) {
  if (e.button !== 0 || view.dead || e.target === view.popover || view.popover.contains(e.target as Node)) return;
  const at = view.stopAtPoint(e.clientX, e.clientY);
  if (at == null) return;
  const anchor = e.shiftKey ? view.field.anchor : at;
  view.select(anchor, at);
  let head = at;
  const move = (m: PointerEvent) => {
    // Not the press itself: cancelling `pointerdown` kills WebKit's click.
    m.preventDefault();
    const next = view.stopAtPoint(m.clientX, m.clientY);
    if (next == null || next === head) return;
    head = next;
    view.select(anchor, head);
  };
  const end = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", end);
    window.removeEventListener("pointercancel", end);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", end);
  window.addEventListener("pointercancel", end);
}

/** The press keeps the keyboard in the textarea: no native selection or
 *  focus moves to what was pressed. */
export function mouseDown(view: MathView, e: MouseEvent) {
  if (e.button !== 0) return;
  e.preventDefault();
  view.focus();
}
