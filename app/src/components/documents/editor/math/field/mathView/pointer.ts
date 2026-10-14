import { extendUnit, type StopRange } from "@/lib/maths/geometry/units";
import type { MathView } from "./index";

interface Press { unit: StopRange | null; group: boolean }
const presses = new WeakMap<MathView, Press>();

/** A press and drag select between stops. Double/triple-click and dragging
 *  use the same atom/group geometry as read-only rendered maths. */
export function pointerDown(view: MathView, e: PointerEvent) {
  if (e.button !== 0 || view.dead || view.popover.contains(e.target as Node)) return;
  const at = view.stopAtPoint(e.clientX, e.clientY);
  if (at == null) return;
  const anchor = e.shiftKey ? view.field.anchor : at;
  const press: Press = { unit: null, group: false };
  presses.set(view, press);
  view.select(anchor, at);
  let head = at;
  const move = (m: PointerEvent) => {
    if (m.pointerId !== e.pointerId || view.dead) return;
    m.preventDefault();
    if (press.unit) {
      const next = view.unitAtPoint(m.clientX, m.clientY, press.group);
      if (next) view.select(...extendUnit(press.unit, next));
      return;
    }
    const next = view.stopAtPoint(m.clientX, m.clientY);
    if (next == null || next === head) return;
    head = next;
    view.select(anchor, head);
  };
  const end = () => {
    if (presses.get(view) === press) presses.delete(view);
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", end);
    window.removeEventListener("pointercancel", end);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", end);
  window.addEventListener("pointercancel", end);
}

/** Keep keyboard focus in the textarea and native selection off the glyphs. */
export function mouseDown(view: MathView, e: MouseEvent) {
  if (e.button !== 0 || view.dead || view.popover.contains(e.target as Node)) return;
  e.preventDefault();
  const press = presses.get(view);
  if (press && e.detail >= 2 && !e.shiftKey) {
    press.group = e.detail >= 3;
    press.unit = view.unitAtPoint(e.clientX, e.clientY, press.group);
    if (press.unit) view.select(...press.unit);
  }
  view.focus();
}
