import { EditorSelection, Prec, type EditorState } from "@codemirror/state";
import { keymap, type Command } from "@codemirror/view";

import { ancestorAt } from "@/components/documents/editor/syntax/syntax";
import { mathContextOf, ownsLines } from "../../mathContext";
import { readsCleanly, visualMathField, visualOf, type VisualMath } from "./visual-state";

/** ←/Backspace from just after inline maths, →/Delete from just before it,
 *  open the field at that end instead of stepping over the rendered maths. */
function enterInline(back: boolean): Command {
  return (view) => {
    const { state } = view;
    const v = state.field(visualMathField, false);
    const { ranges, main } = state.selection;
    if (!v || v.lib !== "ready" || ranges.length !== 1 || !main.empty) return false;
    const node = ancestorAt(
      state,
      main.head,
      (n) => n.name === "InlineMath" || (n.name === "BlockMath" && !ownsLines(state, n)),
      [back ? -1 : 1],
    );
    if (!node || (back ? node.to : node.from) !== main.head) return false;
    const ctx = mathContextOf(node);
    const target = ctx && visualOf(state, ctx);
    if (!target || !readsCleanly(state.sliceDoc(target.from, target.to).trim(), target.display)) return false;
    view.dispatch({ selection: { anchor: back ? target.to : target.from }, scrollIntoView: true });
    return true;
  };
}

/** The display block the field can take on the line just above (`up`) or
 *  below the one holding `pos`. */
function blockBeside(state: EditorState, pos: number, up: boolean): VisualMath | null {
  const ln = state.doc.lineAt(pos);
  if (up ? ln.number === 1 : ln.number === state.doc.lines) return null;
  const next = state.doc.line(ln.number + (up ? -1 : 1));
  const node = ancestorAt(state, up ? next.to : next.from, (n) => n.name === "BlockMath", [up ? -1 : 1]);
  if (!node || !ownsLines(state, node)) return null;
  if (up ? state.doc.lineAt(node.to).number !== next.number : node.from !== next.from) return null;
  const ctx = mathContextOf(node);
  const target = ctx && visualOf(state, ctx);
  return target && readsCleanly(state.sliceDoc(target.from, target.to).trim(), true) ? target : null;
}

/** Up to a display block from the line beside it. ↑/↓ off the edge line go
 *  into the field, since block widgets don't hold the caret and vertical
 *  motion would step over them. ← at a line's start or → at its end, and
 *  Backspace/Delete (`deleting`) from a line with text, which would join it
 *  onto the `$$`, stop at the block's edge, beside its rendering, where
 *  Enter or typing adds a line; the same key again enters the field
 *  (`intoMathBlock` in `live-preview/livePreview/math-blocks.ts`). */
function enterBlock(dir: "up" | "down" | "left" | "right", deleting = false): Command {
  const back = dir === "up" || dir === "left";
  return (view) => {
    const { state } = view;
    const v = state.field(visualMathField, false);
    const { ranges, main } = state.selection;
    if (!v || v.lib !== "ready" || ranges.length !== 1 || !main.empty) return false;
    const ln = state.doc.lineAt(main.head);
    if (dir === "left" || dir === "right") {
      if (main.head !== (back ? ln.from : ln.to) || (deleting && !ln.length)) return false;
    } else {
      const next = view.moveVertically(EditorSelection.cursor(main.head), !back).head;
      if (back ? next >= ln.from : next <= ln.to) return false;
    }
    const target = blockBeside(state, main.head, back);
    if (!target) return false;
    const anchor =
      dir === "left" ? state.doc.lineAt(target.end).to
      : dir === "right" ? target.start
      : back ? target.to : target.from;
    view.dispatch({ selection: { anchor }, scrollIntoView: true });
    return true;
  };
}

export const entryKeys = Prec.highest(
  keymap.of([
    { key: "ArrowLeft", run: (view) => enterInline(true)(view) || enterBlock("left")(view) },
    { key: "Backspace", run: (view) => enterInline(true)(view) || enterBlock("left", true)(view) },
    { key: "ArrowRight", run: (view) => enterInline(false)(view) || enterBlock("right")(view) },
    { key: "Delete", run: (view) => enterInline(false)(view) || enterBlock("right", true)(view) },
    { key: "ArrowUp", run: enterBlock("up") },
    { key: "ArrowDown", run: enterBlock("down") },
  ]),
);

/** Where the last press on rendered maths landed, as a fraction of its
 *  `.ML__latex` box, so the field can put its caret there once it replaces
 *  the rendering, which it lays out the same (`staticMath`). A KaTeX
 *  rendering's ink stands in, spaced narrower than MathLive's. */
let pressed: { fx: number; fy: number; at: number } | null = null;

export function noteMathPress(x: number, y: number, rendered: HTMLElement) {
  const ml = rendered.querySelector(".ML__latex");
  const ink = ml
    ? [ml.getBoundingClientRect()]
    : [...rendered.querySelectorAll(".katex-html > .katex-base")].map((b) => b.getBoundingClientRect());
  const r = ink.length ? ink : [rendered.getBoundingClientRect()];
  const left = Math.min(...r.map((b) => b.left));
  const top = Math.min(...r.map((b) => b.top));
  const width = Math.max(...r.map((b) => b.right)) - left;
  const height = Math.max(...r.map((b) => b.bottom)) - top;
  pressed = { fx: width ? (x - left) / width : 0, fy: height ? (y - top) / height : 0.5, at: Date.now() };
}

/** The last press, which a field mounting takes (once) to place its caret. */
export function takePress(): { fx: number; fy: number; at: number } | null {
  const press = pressed;
  pressed = null;
  return press;
}
