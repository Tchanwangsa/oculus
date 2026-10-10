import { StateEffect, StateField, type EditorState } from "@codemirror/state";
import { showTooltip, type EditorView, type Rect, type Tooltip, type TooltipView, type ViewUpdate } from "@codemirror/view";

import { ancestorAt } from "@/components/documents/editor/syntax/syntax";
import { mathContextOf } from "../../mathContext";
import { CLOSE_ICON, button, el } from "./dom";
import { shapeChange, shapeToggle } from "./shape";
import type { MathRange } from "./state";

/** How long the paste chip stays after its last use, pointer off it. */
const PASTED_LINGER = 5000;

/** Offer the shape switch on the maths starting at a position, or close it. */
export const pastedMath = StateEffect.define<number | null>();

/** The pasted maths' start and its chip, until it is dismissed, times out or
 *  the note is edited other than through the chip. */
export const pastedField = StateField.define<{ pos: number; tooltip: Tooltip } | null>({
  create: () => null,
  update(prev, tr) {
    for (const e of tr.effects) {
      if (e.is(pastedMath)) return e.value == null ? null : { pos: e.value, tooltip: { pos: e.value, create: createPasted } };
    }
    return tr.docChanged ? null : prev;
  },
  provide: (f) => showTooltip.from(f, (v) => v?.tooltip ?? null),
});

/** One `create`, so CodeMirror keeps the chip's DOM (and its timer) as a
 *  switch moves it onto the rewritten maths. */
const createPasted = (view: EditorView): TooltipView => new PastedView(view);

function mathStartingAt(state: EditorState, pos: number): MathRange | null {
  const node = ancestorAt(state, pos, (n) => (n.name === "InlineMath" || n.name === "BlockMath") && n.from === pos, [1]);
  const ctx = node && mathContextOf(node);
  return ctx && { from: ctx.from, to: ctx.to, display: ctx.display, nodeFrom: ctx.start, nodeTo: ctx.end };
}

/** After a paste of maths ending at `end`: the chip that switches it
 *  between inline and block, when `shapeToggle` offers a switch. */
export function offerShapeSwitch(view: EditorView, end: number) {
  const { state } = view;
  const node = ancestorAt(state, end, (n) => (n.name === "InlineMath" || n.name === "BlockMath") && n.to === end, [-1]);
  const math = node && mathStartingAt(state, node.from);
  if (math && shapeToggle(state, math)) view.dispatch({ effects: pastedMath.of(math.nodeFrom) });
}

/**
 * The chip under pasted maths: "Convert to block" or "Convert to inline",
 * and a close button. A switch keeps it up, offering the way back; it goes
 * `PASTED_LINGER` after its last use with the pointer off it, on Esc, or
 * with any other edit.
 */
class PastedView implements TooltipView {
  dom = el("div", "cm-math-tools cm-math-quick cm-math-pasted");
  private shape = button("cm-math-pill cm-math-shape", "", () => this.toggle());
  private timer = 0;
  private hovered = false;

  constructor(readonly view: EditorView) {
    this.dom.setAttribute("role", "group");
    this.dom.setAttribute("aria-label", "Pasted maths");
    this.dom.addEventListener("mousedown", (e) => e.preventDefault());
    this.dom.addEventListener("pointerenter", () => {
      this.hovered = true;
      window.clearTimeout(this.timer);
    });
    this.dom.addEventListener("pointerleave", () => {
      this.hovered = false;
      this.linger();
    });
    const close = button("cm-math-close", "Dismiss", () => view.dispatch({ effects: pastedMath.of(null) }));
    close.innerHTML = CLOSE_ICON;
    this.dom.append(this.shape, close);
    this.sync(view.state);
    this.linger();
  }

  private math(state: EditorState): MathRange | null {
    const pasted = state.field(pastedField, false);
    return pasted ? mathStartingAt(state, pasted.pos) : null;
  }

  private sync(state: EditorState) {
    const math = this.math(state);
    const shape = math && shapeToggle(state, math);
    this.shape.hidden = !shape;
    const label = shape === "block" ? "Convert to block" : "Convert to inline";
    if (this.shape.textContent !== label) {
      this.shape.textContent = label;
      this.shape.title = label;
      this.shape.setAttribute("aria-label", label);
    }
  }

  update(u: ViewUpdate) {
    if (u.docChanged) this.sync(u.state);
  }

  /** Switch the shape, the caret just after the maths as the paste left it. */
  private toggle() {
    const { state } = this.view;
    const math = this.math(state);
    const change = math && shapeChange(state, math);
    if (!change) return;
    this.view.dispatch({
      changes: change.changes,
      selection: { anchor: change.end },
      effects: pastedMath.of(change.start),
      scrollIntoView: true,
      userEvent: "input",
    });
    this.view.focus();
    this.linger();
  }

  private linger() {
    window.clearTimeout(this.timer);
    if (this.hovered) return;
    this.timer = window.setTimeout(() => this.view.dispatch({ effects: pastedMath.of(null) }), PASTED_LINGER);
  }

  /** Under the maths, at its left edge. */
  getCoords(pos: number): Rect {
    const math = this.math(this.view.state);
    const start = this.view.coordsAtPos(pos, 1);
    const end = math ? this.view.coordsAtPos(math.nodeTo, -1) : start;
    // Null hides the tooltip, as for maths scrolled out of view.
    if (!start || !end) return null as unknown as Rect;
    const width = this.dom.offsetWidth;
    return { left: start.left, right: start.left + width, top: start.top, bottom: Math.max(start.bottom, end.bottom) };
  }

  destroy() {
    window.clearTimeout(this.timer);
  }
}
