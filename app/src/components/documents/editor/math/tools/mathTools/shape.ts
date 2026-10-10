import type { EditorState } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { ancestorAt } from "@/components/documents/editor/syntax/syntax";
import { activeMathField } from "../../field/mathField";
import { continuation } from "../../mathContext";
import { mathToolsField, openMathTools, type MathRange } from "./state";

/** What the switch offers for the caret's maths: block for inline maths,
 *  inline for a block with LaTeX in it, nothing inside a table row (a block
 *  would break it). */
export function shapeToggle(state: EditorState, math: MathRange): "block" | "inline" | null {
  if (ancestorAt(state, math.nodeFrom, (n) => n.name === "Table")) return null;
  if (!math.display) return "block";
  return state.sliceDoc(math.from, math.to).trim() ? "inline" : null;
}

/** The caret's maths as a block on lines of its own, or as inline maths
 *  (`shapeChange`). The caret lands at the end of the LaTeX. */
export function toggleShape(view: EditorView, dollars = false) {
  // The field's last keystrokes reach the note first.
  activeMathField(view)?.flush();
  const tools = view.state.field(mathToolsField, false);
  const change = tools?.math && shapeChange(view.state, tools.math, dollars);
  if (!change) return;
  // The rewritten maths starts elsewhere; the popover moves onto it.
  const effects = tools.open ? openMathTools.of(tools.open.kind) : [];
  view.dispatch({ changes: change.changes, selection: { anchor: change.caret }, effects, scrollIntoView: true, userEvent: "input" });
  view.focus();
}

/** A second `$` in a bare inline pair (`$|$`, in the note or its empty
 *  field): the pair becomes an empty block with the caret on its line. */
export function emptyPairToBlock(view: EditorView): boolean {
  const math = view.state.field(mathToolsField, false)?.math;
  if (!math || math.display || math.from !== math.to || view.state.selection.ranges.length !== 1) return false;
  toggleShape(view, true);
  return true;
}

/** Rewrite maths as a block on lines of its own (splitting the text around
 *  it) or as inline maths, keeping `\(`/`\[` or `$` delimiters unless
 *  `dollars` asks for `$` ones: the change,
 *  the end of the LaTeX (`caret`) and the new maths' span. Null when
 *  `shapeToggle` offers nothing. */
export function shapeChange(
  state: EditorState,
  math: MathRange,
  dollars = false,
): { changes: { from: number; to: number; insert: string }; caret: number; start: number; end: number } | null {
  const shape = shapeToggle(state, math);
  if (!shape) return null;
  const bracket = !dollars && state.sliceDoc(math.nodeFrom, math.nodeFrom + 1) === "\\";
  const line = state.doc.lineAt(math.nodeFrom);
  const prefix = continuation(line.text);
  let from = math.nodeFrom;
  let to = math.nodeTo;
  let insert: string;
  let caret: number;
  let start: number;
  let end: number;
  if (shape === "block") {
    const latex = state.sliceDoc(math.from, math.to).trim();
    const nl = `\n${prefix}`;
    const before = state.sliceDoc(line.from, math.nodeFrom);
    const after = state.sliceDoc(math.nodeTo, state.doc.lineAt(math.nodeTo).to);
    insert = "";
    if (before.slice(prefix.length).trim()) {
      from = line.from + before.trimEnd().length;
      insert = nl;
    }
    start = from + insert.length;
    insert += `${bracket ? "\\[" : "$$"}${nl}${latex}`;
    caret = from + insert.length;
    insert += `${nl}${bracket ? "\\]" : "$$"}`;
    end = from + insert.length;
    const rest = after.trimStart();
    if (rest) {
      to = math.nodeTo + after.length - rest.length;
      insert += nl;
    }
  } else {
    // The block's later lines carry the container's markup; inline maths
    // is one line.
    const trimmed = prefix.trimEnd();
    const latex = state
      .sliceDoc(math.from, math.to)
      .split("\n")
      .map((l) => (l.startsWith(prefix) ? l.slice(prefix.length) : l.startsWith(trimmed) ? l.slice(trimmed.length) : l).trim())
      .filter(Boolean)
      .join(" ");
    insert = `${bracket ? "\\(" : "$"}${latex}`;
    let trail = bracket ? "\\)" : "$";
    // A paragraph line right above or below, in the same container, takes
    // the maths back into its sentence.
    const { doc } = state;
    const block = ancestorAt(state, math.nodeFrom, (n) => n.name === "BlockMath", [1]);
    const paragraphAt = (pos: number, side: -1 | 1) => {
      const p = ancestorAt(state, pos, (n) => n.name === "Paragraph", [side]);
      return p?.parent && block?.parent && p.parent.name === block.parent.name && p.parent.from === block.parent.from;
    };
    const first = doc.lineAt(math.nodeFrom);
    const last = doc.lineAt(math.nodeTo);
    if (block && !state.sliceDoc(first.from, math.nodeFrom).slice(prefix.length).trim() && first.number > 1) {
      const above = doc.line(first.number - 1);
      if (above.text.trim() && paragraphAt(above.to, -1)) {
        from = above.from + above.text.trimEnd().length;
        insert = ` ${insert}`;
      }
    }
    if (block && !state.sliceDoc(math.nodeTo, last.to).trim() && last.number < doc.lines) {
      const below = doc.line(last.number + 1);
      const start = below.from + continuation(below.text).length;
      if (below.text.trim() && paragraphAt(start, 1)) {
        to = start;
        trail += " ";
      }
    }
    caret = from + insert.length;
    start = from + (insert.startsWith(" ") ? 1 : 0);
    end = caret + (bracket ? 2 : 1);
    insert += trail;
  }
  return { changes: { from, to, insert }, caret, start, end };
}
