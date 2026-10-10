import { isolateHistory } from "@codemirror/commands";
import { EditorSelection, Transaction } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { focusNote, keepPosVisible } from "./noteScroll";
import type { ActiveMath } from "./mathField/visual-state";
import { fieldWrite, minimalChange, squeezeBlankLines } from "./mathFieldEdits";

/**
 * What the visual field (`rustField`) does to the note around its maths:
 * leave it, add a line beside a block, remove it, paste prose beside it,
 * tidy a block's blank lines as it opens. `target` is the maths
 * the field may write to (`writableSpan`), null when it can't.
 */

export type Direction = "forward" | "backward" | "upward" | "downward";

/** Back to the note, the caret just outside the maths on that side, never
 *  editing it: from a block onto the line before or after, which opens a
 *  touching block's field at its near end (its edge is in it); with nothing
 *  past the note's start or end, the field stays. The page stays put unless
 *  that caret is off screen (`keepPosVisible`). */
export function leaveMaths(view: EditorView, target: ActiveMath | null, dir: Direction) {
  if (!target) {
    focusNote(view);
    return;
  }
  const { doc } = view.state;
  const ahead = dir === "forward" || dir === "downward";
  let anchor: number;
  if (target.block) {
    const first = doc.lineAt(target.start);
    const last = doc.lineAt(target.end);
    if (ahead ? last.to >= doc.length : first.from === 0) return;
    anchor = ahead ? last.to + 1 : first.from - 1;
  } else {
    anchor = ahead ? target.end : target.start;
    if (dir === "upward" || dir === "downward") {
      const moved = view.moveVertically(EditorSelection.cursor(anchor), ahead).head;
      if (moved < target.start || moved > target.end) anchor = moved;
    }
  }
  view.dispatch({ selection: { anchor }, userEvent: "select" });
  focusNote(view);
  keepPosVisible(view, anchor);
}

/** Enter in a block's field: an empty line after the block (`before`: the
 *  field's caret at its very start, before it), the note's caret on it, as
 *  Enter makes a line in text. An undo step of its own. */
export function newlineBeside(view: EditorView, target: ActiveMath | null, before: boolean) {
  if (!target?.block) return;
  const { doc } = view.state;
  const at = before ? doc.lineAt(target.start).from : doc.lineAt(target.end).to;
  const anchor = before ? at : at + 1;
  view.dispatch({
    changes: { from: at, insert: "\n" },
    selection: { anchor },
    annotations: isolateHistory.of("full"),
    userEvent: "input",
  });
  focusNote(view);
  keepPosVisible(view, anchor);
}

/** Backspace in an empty field takes the maths (a block's lines) away. */
export function removeMaths(view: EditorView, target: ActiveMath | null) {
  if (!target) return;
  const { doc } = view.state;
  let from = target.start;
  let to = target.end;
  if (target.block) {
    from = doc.lineAt(target.start).from;
    to = doc.lineAt(target.end).to;
    if (to < doc.length) to++;
    else if (from > 0) from--;
  }
  view.dispatch({ changes: { from, to }, selection: { anchor: from }, userEvent: "delete" });
  focusNote(view);
}

/** Pasted text that is all maths: one delimited `$…$`, `$$…$$`, `\(…\)` or
 *  `\[…\]`, or LaTeX with no delimiters in it. */
export function mathOnly(text: string): boolean {
  const t = text.trim();
  if (!/\$|(?<!\\)\\[([]/.test(t)) return true;
  return (
    /^\$\$(?:(?!\$\$)[\s\S])*\$\$$/.test(t) ||
    /^\$[^$]+\$$/.test(t) ||
    /^\\\[(?:(?!\\\])[\s\S])*\\\]$/.test(t) ||
    /^\\\((?:(?!\\\))[\s\S])*\\\)$/.test(t)
  );
}

/** Markdown with prose around maths can't live inside maths: it goes into
 *  the note just after this maths (a block's on a line of its own). */
export function pasteBeside(view: EditorView, target: ActiveMath | null, text: string) {
  if (!target) return;
  const at = target.block ? view.state.doc.lineAt(target.end).to : target.end;
  const insert = target.block ? `\n${text.replace(/^\n+/, "")}` : text;
  view.dispatch({
    changes: { from: at, insert },
    selection: { anchor: at + insert.length },
    scrollIntoView: true,
    userEvent: "input.paste",
  });
  focusNote(view);
}

/** Blank lines in a block's LaTeX (an older note's, another editor's) go
 *  as a field opens on it, outside the history; the fields' own writes
 *  never add them (`squeezeBlankLines`). `opening` runs before the write
 *  with the maths' new trimmed LaTeX, so the field knows the write as its
 *  own. Returns that LaTeX, or null when nothing changed. */
export function dropBlankLines(
  view: EditorView,
  target: ActiveMath | null,
  id: number,
  opening: (latex: string) => void,
): string | null {
  const current = target ? view.state.sliceDoc(target.from, target.to) : "";
  if (!target || !current.includes("\n") || !current.trim()) return null;
  const tidied = `\n${squeezeBlankLines(current.trim())}\n`;
  const change = minimalChange(current, tidied, target.from);
  if (!change) return null;
  opening(tidied.trim());
  view.dispatch({ changes: change, annotations: [fieldWrite.of(id), Transaction.addToHistory.of(false)] });
  return tidied.trim();
}
