import { EditorSelection, type EditorState, type SelectionRange, type StateCommand } from "@codemirror/state";

import { BOLD, CODE, ITALIC, LINK, STRIKE, trimmed, unwrapWithin, wrapWithin, type MarkKind } from "./inlineMarks";
import { enclosing } from "./shared";

/** Takes `kind` off the selected text only, when `range` sits inside one
 *  (`unwrapWithin`); null when it does not. */
function unmark(state: EditorState, range: SelectionRange, kind: MarkKind) {
  const core = trimmed(state, range);
  const node = enclosing(state, core, kind.node);
  const wrap = node && kind.wrap(node);
  if (!wrap) return null;
  const changes = state.changes(unwrapWithin(state, core, wrap, kind.hugs));
  return { changes, range: range.map(changes) };
}

function toggleInline(marker: string, kind: MarkKind): StateCommand {
  return ({ state, dispatch }) => {
    const tr = state.changeByRange((range) => {
      const off = unmark(state, range, kind);
      if (off) return off;
      if (range.empty) {
        return {
          changes: { from: range.from, insert: marker + marker },
          range: EditorSelection.cursor(range.from + marker.length),
        };
      }
      const changes = state.changes(wrapWithin(state, range, kind, marker));
      return { changes, range: range.map(changes) };
    });
    dispatch(state.update(tr, { scrollIntoView: true, userEvent: "input" }));
    return true;
  };
}

export const toggleBold = toggleInline("**", BOLD);
export const toggleItalic = toggleInline("*", ITALIC);
export const toggleStrike = toggleInline("~~", STRIKE);
export const toggleCode = toggleInline("`", CODE);

/** `[sel](|)`, caret in the URL; an empty selection gets `[|]()`, a selected
 *  URL `[|](url)`. Inside a link, unlinks the selected text. */
export const toggleLink: StateCommand = ({ state, dispatch }) => {
  const tr = state.changeByRange((range) => {
    const off = unmark(state, range, LINK);
    if (off) return off;
    const text = state.sliceDoc(range.from, range.to);
    if (/^(https?:\/\/|www\.)\S+$/i.test(text)) {
      return {
        changes: { from: range.from, to: range.to, insert: `[](${text})` },
        range: EditorSelection.cursor(range.from + 1),
      };
    }
    const insert = `[${text}]()`;
    return {
      changes: { from: range.from, to: range.to, insert },
      range: EditorSelection.cursor(range.from + (text ? insert.length - 1 : 1)),
    };
  });
  dispatch(state.update(tr, { scrollIntoView: true, userEvent: "input" }));
  return true;
};
