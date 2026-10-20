import { EditorView } from "@codemirror/view";

import { ancestorAt } from "../../syntax/syntax";
import { clipboardGrid, gridToMarkdown } from "../tableGrid";
import { edgePadding } from "./edges";

/** A spreadsheet's or web page's table, pasted into the note, becomes a pipe
 *  table. It starts on a line of its own after a blank line, since GFM
 *  reads a table straight under text with a pipe in it as part of that
 *  text; `edgePaste` pads it beside another table and under it. */
export const gridPaste = EditorView.domEventHandlers({
  paste(e, view) {
    const { state } = view;
    if (state.readOnly || state.selection.ranges.length !== 1) return false;
    const grid = clipboardGrid(e.clipboardData, true);
    if (!grid) return false;
    const { from, to } = state.selection.main;
    const code = (n: { name: string }) => n.name === "FencedCode" || n.name === "CodeBlock" || n.name === "InlineCode";
    if (ancestorAt(state, from, code)) return false;
    e.preventDefault();
    const markdown = gridToMarkdown(grid);
    let head = "";
    if (!edgePadding(state, from)) {
      const line = state.doc.lineAt(from);
      if (state.sliceDoc(line.from, from).trim()) head = "\n\n";
      else if (line.number > 1 && state.doc.line(line.number - 1).text.trim()) head = "\n";
    }
    view.dispatch({
      changes: { from, to, insert: head + markdown },
      selection: { anchor: from + head.length + markdown.length },
      scrollIntoView: true,
      userEvent: "input.paste",
    });
    return true;
  },
});
