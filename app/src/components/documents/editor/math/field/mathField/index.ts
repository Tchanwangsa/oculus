import { EditorState, Transaction, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { ancestorAt } from "@/components/documents/editor/syntax/syntax";
import { mathContextOf } from "../../mathContext";
import { minimalChange, selectionPastField, withoutEndRows } from "../mathFieldEdits";
import { entryKeys } from "./keys";
import { visualMath, visualMathField } from "./visual-state";

/**
 * Visual maths in Live mode: while the caret is in a maths node, a visual
 * field stands in for it and is where it is edited — slots for a
 * fraction's parts, `\` commands that become one symbol. It is the Rust
 * field (`math/field/rustField`: `MathView` over the maths engine's edit
 * model), a `VisualField` to the rest of the editor. Each edit in the field
 * rewrites only the LaTeX between the delimiters; opening it writes
 * nothing. Maths the field can't read cleanly, and maths switched to TeX
 * from the toolbox, are typed as LaTeX source (`tools/mathTools`). Until
 * the maths engine has loaded, maths renders as a placeholder
 * (`live-preview/widgets/math.ts`).
 */

export { MATH_ARRAYSTRETCH, MATH_LINE_GAP } from "./layout";
export { noteMathPress } from "./keys";
export { activeMathField, fieldKeys, type VisualField } from "./registry";
export { layoutBlock } from "./serialize";
export {
  fieldReady,
  readsCleanly,
  setMathMode,
  touchedMath,
  visualMath,
  visualMathField,
  visualToggle,
  type ActiveMath,
  type VisualMath,
} from "./visual-state";
export { MathFieldWidget } from "./widget";

/** A selection extended from inside the open field to past its maths starts
 *  at the maths' edge (`selectionPastField`). */
const fieldSelection = EditorState.transactionFilter.of((tr) => {
  if (tr.docChanged || !tr.selection || !tr.isUserEvent("select")) return tr;
  const v = visualMath(tr.startState);
  const sel = v && selectionPastField(tr.startState.doc, v, tr.selection);
  return sel ? [tr, { selection: sel, sequential: true }] : tr;
});

/** A block the field leaves loses empty rows at its end (Enter past its last
 *  line), which would draw as a blank line under the formula, where the
 *  caret beside the block then rests. Outside the history, after the update
 *  that closed the field. */
const dropEndRows = EditorView.updateListener.of((u) => {
  const left = visualMath(u.startState);
  if (!left?.block || visualMath(u.state)?.id === left.id) return;
  const start = u.changes.mapPos(left.start, 1);
  queueMicrotask(() => {
    const { state } = u.view;
    const node = ancestorAt(state, start, (n) => n.name === "BlockMath", [1]);
    const ctx = node && node.from === start ? mathContextOf(node) : null;
    if (!ctx || visualMath(state)?.start === ctx.start) return;
    const latex = state.sliceDoc(ctx.from, ctx.to);
    const change = minimalChange(latex, withoutEndRows(latex), ctx.from);
    if (change) u.view.dispatch({ changes: change, annotations: Transaction.addToHistory.of(false) });
  });
});

/** Inline maths left empty (`$$`, `$ $`, `\(\)`) goes when the field closes
 *  with the caret outside it, rather than staying as an invisible pair. A
 *  caret still inside (TeX mode, the window losing focus) keeps it. Outside
 *  the history, as `dropEndRows`. */
const dropEmptyInline = EditorView.updateListener.of((u) => {
  const left = visualMath(u.startState);
  if (!left || left.block || visualMath(u.state)?.id === left.id) return;
  const start = u.changes.mapPos(left.start, 1);
  const end = u.changes.mapPos(left.end, -1);
  queueMicrotask(() => {
    const { state } = u.view;
    if (end > state.doc.length || !/^(?:\$\s*\$|\\\(\s*\\\))$/.test(state.sliceDoc(start, end))) return;
    if (state.selection.ranges.some((r) => r.to > start && r.from < end)) return;
    u.view.dispatch({ changes: { from: start, to: end }, annotations: Transaction.addToHistory.of(false) });
  });
});

/** The visual-maths state and the keys into a field. Live mode
 *  only; the decorations are `live-preview/`'s. */
export function mathField(): Extension {
  return [visualMathField, entryKeys, fieldSelection, dropEndRows, dropEmptyInline];
}
