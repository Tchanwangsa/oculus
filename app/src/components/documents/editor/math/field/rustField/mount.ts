import { takePress } from "../mathField/keys";
import type { RustFieldController } from "./controller";

/** How long after a press on the rendering the field opening takes it. */
const PRESS_WINDOW = 600;

/** The ink of the view's maths: its KaTeX rows, as `noteMathPress`
 *  measured the rendering the press landed on. */
function inkBox(root: HTMLElement): DOMRect | null {
  const rows = [...root.querySelectorAll(".katex-html > .katex-base")]
    .map((b) => b.getBoundingClientRect())
    .filter((r) => r.width > 0);
  const r = rows.length ? rows : [root.getBoundingClientRect()];
  const left = Math.min(...r.map((b) => b.left));
  const top = Math.min(...r.map((b) => b.top));
  const right = Math.max(...r.map((b) => b.right));
  const bottom = Math.max(...r.map((b) => b.bottom));
  return right > left ? new DOMRect(left, top, right - left, bottom - top) : null;
}

/** The field's first caret: where the rendering was pressed (`takePress`,
 *  mapped onto the view's maths), the whole maths when the note's selection
 *  held it, else the end the note's caret came from. */
export function placeCaret(ctl: RustFieldController) {
  const { mv, view } = ctl;
  const target = ctl.target();
  const sel = view.state.selection.main;
  const pressed = takePress();
  const ink = pressed && Date.now() - pressed.at < PRESS_WINDOW ? inkBox(mv.rendered) : null;
  const stop = pressed && ink ? mv.stopAtPoint(ink.left + pressed.fx * ink.width, ink.top + pressed.fy * ink.height) : null;
  if (stop != null) mv.select(stop, stop);
  else if (target && !sel.empty && sel.from <= target.from && sel.to >= target.to) mv.run("selectAll");
  else if (target && sel.head <= target.from) mv.select(0, 0);
}

/** A press in a block's padding, above or below its maths: the caret rests
 *  before or after the block, as on its rendering. */
export function pressBeside(ctl: RustFieldController, e: MouseEvent) {
  if (e.button !== 0 || e.target !== ctl.dom || !ctl.block) return;
  e.preventDefault();
  const target = ctl.target();
  if (!target) return;
  const above = e.clientY < ctl.mv.frame.getBoundingClientRect().top;
  const anchor = above ? target.start : ctl.view.state.doc.lineAt(target.end).to;
  ctl.view.dispatch({ selection: { anchor }, userEvent: "select.pointer" });
  ctl.view.focus();
}
