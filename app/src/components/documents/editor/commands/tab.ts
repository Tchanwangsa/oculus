import { indentLess, indentMore } from "@codemirror/commands";
import { deleteMarkupBackward, insertNewlineContinueMarkup } from "@codemirror/lang-markdown";
import type { Command, KeyBinding } from "@codemirror/view";

import { toggleBold, toggleCode, toggleItalic, toggleLink, toggleStrike } from "./marks";

/** A list item's line, by its text. */
const LIST_LINE = /^[ \t]*(?:[-*+]|\d+[.)])[ \t]/;

/** Tab indents a list item, or every line of a multi-line selection, by two
 *  spaces; elsewhere it types two spaces. */
export const indentOrTab: Command = (view) => {
  const { state } = view;
  const multiline = state.selection.ranges.some(
    (r) => state.doc.lineAt(r.from).number !== state.doc.lineAt(r.to).number,
  );
  if (multiline || LIST_LINE.test(state.doc.lineAt(state.selection.main.head).text)) {
    return indentMore(view);
  }
  view.dispatch(state.replaceSelection("  "), { scrollIntoView: true, userEvent: "input" });
  return true;
};

export const noteKeymap: KeyBinding[] = [
  { key: "Mod-b", run: toggleBold },
  { key: "Mod-i", run: toggleItalic },
  { key: "Mod-Shift-x", run: toggleStrike },
  { key: "Mod-e", run: toggleCode },
  { key: "Mod-k", run: toggleLink },
  { key: "Tab", run: indentOrTab, shift: indentLess },
  { key: "Enter", run: insertNewlineContinueMarkup },
  { key: "Backspace", run: deleteMarkupBackward },
];
