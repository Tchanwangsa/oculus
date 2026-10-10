import { caretAt, wholeStructures } from "../geometry";
import { modelOf } from "../model";
import type { FieldController } from "./field-controller";

/** A press in a block's padding, above or below the shaded field: the
 *  caret rests before or after the block, as on its rendering. */
export function pressBeside(ctl: FieldController, e: MouseEvent) {
  if (e.button !== 0 || e.target !== ctl.dom || !ctl.block) return;
  e.preventDefault();
  ctl.flush();
  const target = ctl.target();
  if (!target) return;
  const above = e.clientY < ctl.mf.getBoundingClientRect().top;
  const anchor = above ? target.start : ctl.view.state.doc.lineAt(target.end).to;
  ctl.view.dispatch({ selection: { anchor }, userEvent: "select.pointer" });
  ctl.view.focus();
}

/** A plain click: our caret (`caretAt`) over the one MathLive placed. */
export function press(ctl: FieldController, e: PointerEvent) {
  if (e.button !== 0 || e.detail > 1 || e.shiftKey || !ctl.mf.selectionIsCollapsed) return;
  const fixed = caretAt(ctl.mf, e.clientX, e.clientY);
  if (fixed != null && fixed !== ctl.mf.position) ctl.mf.position = fixed;
}

/** A selection dragged or extended into or out of a structure takes it
 *  whole (`wholeStructures`), keeping the end being moved as the caret. */
export function selectionChanged(ctl: FieldController) {
  const model = modelOf(ctl.mf);
  const { ranges } = ctl.mf.selection;
  if (!model || ranges.length !== 1 || ranges[0][0] === ranges[0][1]) return;
  const [start, end] = ranges[0][0] < ranges[0][1] ? ranges[0] : [ranges[0][1], ranges[0][0]];
  const whole = wholeStructures(model, start, end);
  if (!whole) return;
  const backward = ctl.mf.position === start;
  ctl.mf.selection = { ranges: [whole], direction: backward ? "backward" : "forward" };
}
