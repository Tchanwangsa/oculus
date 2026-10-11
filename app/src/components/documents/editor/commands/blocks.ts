import { EditorSelection, type ChangeSpec, type EditorState, type StateCommand } from "@codemirror/state";

import { enclosing, enclosingBlock, type Dispatch } from "./shared";

/**
 * Insert `text` as a block of its own at the main selection, blank lines
 * around it (so a picture is not drawn inline and `---` is not read as a
 * heading underline). `caret` is an offset into `text`, or a range in it;
 * by default the caret lands after the block.
 */
function insertBlock(
  state: EditorState,
  dispatch: Dispatch,
  text: string,
  caret?: number | [number, number],
): boolean {
  const { from, to } = state.selection.main;
  const before = state.sliceDoc(0, from);
  const after = state.sliceDoc(to);
  const prefix = before === "" || before.endsWith("\n\n") ? "" : before.endsWith("\n") ? "\n" : "\n\n";
  const suffix = after.startsWith("\n\n") ? "" : after.startsWith("\n") ? "\n" : "\n\n";
  const insert = `${prefix}${text}${suffix}`;
  const start = from + prefix.length;
  const selection =
    caret === undefined ? EditorSelection.cursor(from + insert.length)
    : typeof caret === "number" ? EditorSelection.cursor(start + caret)
    : EditorSelection.range(start + caret[0], start + caret[1]);
  dispatch(
    state.update({
      changes: { from, to, insert },
      selection,
      scrollIntoView: true,
      userEvent: "input",
    }),
  );
  return true;
}

/** A picture's markdown, own block, caret after it. */
export function insertImage(markdown: string): StateCommand {
  return ({ state, dispatch }) => insertBlock(state, dispatch, markdown);
}

export const insertDivider: StateCommand = ({ state, dispatch }) => insertBlock(state, dispatch, "---");

/** A GFM table, `rows` counting the header, caret after it so Live mode
 *  draws the grid at once. */
export function insertTable(rows: number, cols: number): StateCommand {
  return ({ state, dispatch }) => {
    const header = `| ${Array.from({ length: cols }, (_, i) => `Column ${i + 1}`).join(" | ")} |`;
    const rule = `|${" --- |".repeat(cols)}`;
    const body = Array.from({ length: Math.max(rows - 1, 1) }, () => `|${"   |".repeat(cols)}`);
    const text = [header, rule, ...body].join("\n");
    return insertBlock(state, dispatch, text);
  };
}

/** Wrap the selected lines in a fence, or open an empty one; inside a fence,
 *  remove it. */
export const toggleCodeBlock: StateCommand = ({ state, dispatch }) => {
  const main = state.selection.main;
  const fence = enclosingBlock(state, main.head, "FencedCode");
  if (fence) {
    const open = state.doc.lineAt(fence.from);
    const close = state.doc.lineAt(fence.to);
    const marks = fence.getChildren("CodeMark");
    const closed = marks.length > 1 && close.number > open.number;
    const changes: ChangeSpec[] = [{ from: open.from, to: Math.min(open.to + 1, state.doc.length) }];
    if (closed) {
      // The closing line with its newline; the last line takes the one before.
      const atEnd = close.to === state.doc.length && close.from - 1 > open.to;
      changes.push({ from: atEnd ? close.from - 1 : close.from, to: Math.min(close.to + 1, state.doc.length) });
    }
    dispatch(state.update({ changes, scrollIntoView: true, userEvent: "input" }));
    return true;
  }
  const first = state.doc.lineAt(main.from);
  const last = state.doc.lineAt(main.to);
  if (main.empty && first.text.trim() === "") {
    return insertBlock(state, dispatch, "```\n\n```", 4);
  }
  dispatch(
    state.update({
      changes: [
        { from: first.from, insert: "```\n" },
        { from: last.to, insert: "\n```" },
      ],
      scrollIntoView: true,
      userEvent: "input",
    }),
  );
  return true;
};

/** `$|$` inline, or a `$$` block on an empty line; inside inline maths,
 *  removes its delimiters. */
export const insertMath: StateCommand = ({ state, dispatch }) => {
  const main = state.selection.main;
  const inline = enclosing(state, main, "InlineMath");
  if (inline) {
    const marks = inline.getChildren("MathMark");
    if (marks.length >= 2) {
      const changes = state.changes([
        { from: marks[0].from, to: marks[0].to },
        { from: marks[marks.length - 1].from, to: marks[marks.length - 1].to },
      ]);
      dispatch(state.update({ changes, selection: main.map(changes), userEvent: "input" }));
      return true;
    }
  }
  const ln = state.doc.lineAt(main.from);
  if (main.empty && ln.text.trim() === "") {
    return insertBlock(state, dispatch, "$$\n\n$$", 3);
  }
  const text = state.sliceDoc(main.from, main.to);
  dispatch(
    state.update({
      changes: { from: main.from, to: main.to, insert: `$${text}$` },
      selection: text
        ? EditorSelection.range(main.from + 1, main.to + 1)
        : EditorSelection.cursor(main.from + 1),
      scrollIntoView: true,
      userEvent: "input",
    }),
  );
  return true;
};
