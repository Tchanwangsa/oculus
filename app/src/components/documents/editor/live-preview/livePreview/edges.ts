import { EditorSelection, EditorState, Prec } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { continuation, mathAt } from "../../math/mathContext";
import { BLOCK_MATH_TYPE } from "@/lib/markdown/math";
import { emptyPairToBlock, offerShapeSwitch } from "../../math/tools/mathTools";
import { ancestorAt } from "../../syntax/syntax";
import { mathBlockAt } from "./math-blocks";
import { caretOf } from "./shared";
import { tableAt, tableBeside } from "./tables";

/** How text put at `pos` is padded to stay off a drawn block. GFM reads a
 *  line straight under a table as a row, so text at its end or on the empty
 *  line under it starts after a blank line; at its start, it gets its own
 *  line. Text at a maths block's edge gets its own line, or it would join a
 *  `$$` and undo the block. */
function edgePadding(state: EditorState, pos: number): [string, string] | null {
  if (tableAt(state, pos, "to")) return ["\n\n", ""];
  if (tableAt(state, pos, "from")) return ["", "\n"];
  if (!state.doc.lineAt(pos).length && tableBeside(state, pos, true)) return ["\n", ""];
  if (mathBlockAt(state, pos, "to")) return ["\n", ""];
  if (mathBlockAt(state, pos, "from")) return ["", "\n"];
  return null;
}

/** Where text typed at `from` goes: the caret, when it rests at a block's
 *  edge. The browser has no text position there, so it types at the nearest
 *  one, the next line's start or the far side of a block right beside. */
function typedAt(state: EditorState, from: number): number {
  const pos = caretOf(state);
  return pos != null && edgePadding(state, pos) ? pos : from;
}

/** Typing beside a block; a cell's own writes are dispatched, not typed. */
export const edgeTyping = EditorView.inputHandler.of((view, typed, to, text) => {
  if (typed !== to || view.composing) return false;
  const from = typedAt(view.state, typed);
  if (caretOf(view.state) !== from) return false;
  const pad = edgePadding(view.state, from);
  if (!pad) return false;
  view.dispatch({
    changes: { from, insert: pad[0] + text + pad[1] },
    selection: { anchor: from + pad[0].length + text.length },
    scrollIntoView: true,
    userEvent: "input.type",
  });
  return true;
});

/** Characters a typed `$` may sit before and still open maths: the line's
 *  end, a space, or closing punctuation — not the middle of a word. */
const BEFORE_PAIR = /^$|^[\s)\]}.,;:!?]/;

/**
 * `$` opens inline maths at once: `$|$` (`\(|\)` at a line's start), which
 * Live mode draws as an empty field the caret is in. A `$` typed in that pair makes it a block. Not
 * after `\` or `$`, in code, or before the text of a word, so a price or an
 * escaped `\$` stays a character. Ahead of `mathShorthand`'s handler, which
 * would type the `$` into the pair.
 */
export const dollarTyping = Prec.high(EditorView.inputHandler.of((view, typed, to, text) => {
  if (text !== "$" || typed !== to || view.composing || view.state.readOnly) return false;
  const { state } = view;
  const from = typedAt(state, typed);
  if (state.selection.ranges.length !== 1 || caretOf(state) !== from) return false;
  const ctx = mathAt(state, from);
  if (ctx) return ctx.node == null && emptyPairToBlock(view);
  // Beside a block the pair gets a line of its own, so the block's `$$`
  // and the text past it are no neighbours.
  const pad = edgePadding(state, from) ?? ["", ""];
  const before = pad[0] ? "" : state.sliceDoc(from - 1, from);
  if (before === "\\" || before === "$") return false;
  if (!pad[1] && !BEFORE_PAIR.test(state.sliceDoc(from, from + 1))) return false;
  if (ancestorAt(state, from, (n) => n.name === "InlineCode" || n.name === "FencedCode" || n.name === "CodeBlock")) {
    return false;
  }
  // At a line's start `$$` opens a display block in every Markdown reader,
  // which would run to the next `$$`; `\(\)` there is the empty pair, and
  // the field writes it as `$…$` once it holds something (`FieldController.flush`).
  const lineBefore = pad[0] ? "" : state.sliceDoc(state.doc.lineAt(from).from, from);
  const pair = continuation(lineBefore).length === lineBefore.length ? "\\(\\)" : "$$";
  view.dispatch({
    changes: { from, insert: pad[0] + pair + pad[1] },
    selection: { anchor: from + pad[0].length + pair.length / 2 },
    scrollIntoView: true,
    userEvent: "input.type",
  });
  return true;
}));

/** Pasting beside a block, which skips the input handler. */
export const edgePaste = EditorState.transactionFilter.of((tr) => {
  if (!tr.docChanged || !tr.isUserEvent("input.paste")) return tr;
  const pos = caretOf(tr.startState);
  if (pos == null) return tr;
  const parts: { from: number; to: number; text: string }[] = [];
  tr.changes.iterChanges((from, to, _a, _b, inserted) => parts.push({ from, to, text: inserted.toString() }));
  if (parts.length !== 1 || parts[0].from !== pos || parts[0].to !== pos) return tr;
  const pad = edgePadding(tr.startState, pos);
  if (!pad) return tr;
  const { text } = parts[0];
  return {
    changes: { from: pos, insert: pad[0] + text + pad[1] },
    selection: { anchor: pos + pad[0].length + text.length },
    scrollIntoView: true,
    userEvent: "input.paste",
  };
});

/** LaTeX copied from a maths field pasted outside maths keeps rendering
 *  and its shape: a block's copy goes in as a block on lines of its own
 *  (inline in a table row, which a block would break), the rest as `$…$`.
 *  Inside maths' source it stays bare. */
export const fieldLatexPaste = EditorView.domEventHandlers({
  paste(e, view) {
    const data = e.clipboardData;
    const latex = data?.getData("text/plain").trim();
    if (!latex || !data?.types.includes("application/x-latex") || view.state.readOnly) return false;
    const inMaths = (pos: number) => {
      const node = ancestorAt(view.state, pos, (n) => n.name === "InlineMath" || n.name === "BlockMath");
      return node != null && node.from < pos && pos < node.to;
    };
    if (view.state.selection.ranges.some((r) => inMaths(r.from))) return false;
    e.preventDefault();
    const { state } = view;
    const block = data.getData(BLOCK_MATH_TYPE);
    const spec = state.changeByRange((r) => {
      if (!block || ancestorAt(state, r.from, (n) => n.name === "Table")) {
        const insert = `$${latex}$`;
        return { changes: { from: r.from, to: r.to, insert }, range: EditorSelection.cursor(r.from + insert.length) };
      }
      const line = state.doc.lineAt(r.from);
      const prefix = continuation(line.text);
      const nl = `\n${prefix}`;
      const before = state.sliceDoc(line.from, r.from).slice(prefix.length).trim() ? nl : "";
      const after = state.sliceDoc(r.to, state.doc.lineAt(r.to).to).trim() ? nl : "";
      const body = block.replace(/\n/g, nl);
      return {
        changes: { from: r.from, to: r.to, insert: before + body + after },
        range: EditorSelection.cursor(r.from + before.length + body.length),
      };
    });
    view.dispatch(state.update(spec, { scrollIntoView: true, userEvent: "input.paste" }));
    if (view.state.selection.ranges.length === 1) offerShapeSwitch(view, view.state.selection.main.head);
    return true;
  },
});
