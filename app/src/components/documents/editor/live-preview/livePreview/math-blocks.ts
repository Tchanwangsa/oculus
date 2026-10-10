import { syntaxTree } from "@codemirror/language";
import { Prec, StateField, type EditorState, type Range } from "@codemirror/state";
import { Decoration, EditorView, keymap, type Command, type DecorationSet } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";

import { visualMathField } from "../../math/field/mathField";
import { MathWidget } from "../widgets";
import { blockField } from "./blocks";
import { caretOf, mathParts, type Span } from "./shared";

/** The rendered maths among `decos`, inline or blocks. */
export function mathIn(decos: DecorationSet, block: boolean): DecorationSet {
  const out: Range<Decoration>[] = [];
  for (const iter = decos.iter(); iter.value; iter.next()) {
    const w = iter.value.spec.widget;
    if (w instanceof MathWidget && w.block === block) out.push(iter.value.range(iter.from, iter.to));
  }
  return Decoration.set(out);
}

/** Rendered maths is atomic once the field can take it, so a selection
 *  covers it whole and the caret rests only at its edges. Before MathLive
 *  loads, Backspace at its edge would take the whole node. */
export function mathAtoms(view: EditorView, set: DecorationSet | undefined): DecorationSet {
  return view.state.field(visualMathField, false)?.lib === "ready" && set ? set : Decoration.none;
}

export const mathBlockField = StateField.define<DecorationSet>({
  create: (state) => mathIn(state.field(blockField), true),
  update(blocks, tr) {
    const next = tr.state.field(blockField);
    return next === tr.startState.field(blockField, false) ? blocks : mathIn(next, true);
  },
  provide: (f) => EditorView.atomicRanges.of((view) => mathAtoms(view, view.state.field(f))),
});

/** The drawn maths block starting (`from`) or ending (`to`) exactly at `pos`. */
export function mathBlockAt(state: EditorState, pos: number, edge: "from" | "to"): Span | null {
  let found = null as Span | null;
  state.field(mathBlockField, false)?.between(pos, pos, (from, to) => {
    if ((edge === "from" ? from : to) !== pos) return;
    found = { from, to };
    return false;
  });
  return found;
}

/** From the caret before a block (`forward`) or after it, into the field at
 *  that end: →/↓/Delete before it, ←/↑/Backspace after it. */
function intoMathBlock(forward: boolean): Command {
  return (view) => {
    const { state } = view;
    const head = caretOf(state);
    const block = head == null ? null : mathBlockAt(state, head, forward ? "from" : "to");
    const node = block && syntaxTree(state).resolveInner(block.from, 1);
    let math: SyntaxNode | null = node;
    while (math && math.name !== "BlockMath") math = math.parent;
    const parts = math && mathParts(state, math);
    if (!parts) return false;
    view.dispatch({ selection: { anchor: forward ? parts.from : parts.to }, scrollIntoView: true });
    return true;
  };
}

/** Backspace before a block, Delete after it: an empty line beyond goes,
 *  else the caret steps onto that line rather than joining it to a `$$`. */
function deleteBesideMathBlock(forward: boolean): Command {
  return (view) => {
    const { state } = view;
    const head = caretOf(state);
    if (head == null || !mathBlockAt(state, head, forward ? "to" : "from")) return false;
    const ln = state.doc.lineAt(head);
    const n = ln.number + (forward ? 1 : -1);
    if (n < 1 || n > state.doc.lines) return true;
    const beyond = state.doc.line(n);
    if (beyond.length) view.dispatch({ selection: { anchor: forward ? beyond.from : beyond.to }, scrollIntoView: true });
    else {
      const change = forward ? { from: ln.to, to: beyond.to } : { from: beyond.from, to: ln.from };
      view.dispatch({ changes: change, scrollIntoView: true, userEvent: "delete" });
    }
    return true;
  };
}

export const mathBlockKeys = Prec.highest(
  keymap.of([
    { key: "ArrowRight", run: intoMathBlock(true) },
    { key: "ArrowDown", run: intoMathBlock(true) },
    { key: "Delete", run: (view) => intoMathBlock(true)(view) || deleteBesideMathBlock(true)(view) },
    { key: "ArrowLeft", run: intoMathBlock(false) },
    { key: "ArrowUp", run: intoMathBlock(false) },
    { key: "Backspace", run: (view) => intoMathBlock(false)(view) || deleteBesideMathBlock(false)(view) },
  ]),
);
