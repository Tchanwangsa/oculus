import type { EditorState, SelectionRange, Transaction } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

import { ancestorAt } from "../syntax/syntax";

export type Dispatch = (tr: Transaction) => void;

/** The innermost `name` node holding the range. A caret must be strictly
 *  inside, so one just past `**bold**` starts new bold rather than unwrapping. */
export function enclosing(state: EditorState, range: SelectionRange, name: string): SyntaxNode | null {
  return ancestorAt(state, range.from, (node) =>
    node.name === name && (range.empty
      ? node.from < range.from && range.to < node.to
      : node.from <= range.from && range.to <= node.to),
  );
}

/** The innermost `name` block on the line holding `pos`. */
export function enclosingBlock(state: EditorState, pos: number, name: string): SyntaxNode | null {
  const ln = state.doc.lineAt(pos);
  const start = ln.from + (ln.text.length - ln.text.trimStart().length);
  return ancestorAt(state, start, (node) => node.name === name, [1]);
}
