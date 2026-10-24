import { syntaxTree } from "@codemirror/language";
import { StateField, type EditorState, type Range } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";

import { line, touches } from "./shared";

/** Every blank line between top-level blocks. */
function build(state: EditorState): DecorationSet {
  const tree = syntaxTree(state);
  const out: Range<Decoration>[] = [];
  for (let n = 1; n <= state.doc.lines; n++) {
    const blank = state.doc.line(n);
    if (blank.text.trim() !== "" || touches(state, blank.from, blank.to)) continue;
    // A blank line inside a code fence or list is part of that block.
    if (tree.resolve(blank.from, 1).name !== "Document") continue;
    out.push(line("cm-block-gap").range(blank.from));
  }
  return Decoration.set(out);
}

/**
 * The blank line between two paragraphs shrinks to a gap, as the blank lines
 * around a block do (`blocks.ts`), unless the caret is on it — for a field
 * whose rendering spaces paragraphs by a margin rather than a line
 * (`NoteField`'s compact size). Rebuilds on every transaction: a field's text
 * is a few lines.
 */
export const paragraphGaps = StateField.define<DecorationSet>({
  create: build,
  update: (gaps, tr) => (tr.docChanged || tr.selection || tr.effects.length ? build(tr.state) : gaps),
  provide: (field) => EditorView.decorations.from(field),
});
