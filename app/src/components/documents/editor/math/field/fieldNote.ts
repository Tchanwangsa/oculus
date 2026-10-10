import { EditorSelection, Transaction, type ChangeSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import type { ActiveMath } from "./mathField/visual-state";
import { fieldWrite, minimalChange, squeezeBlankLines } from "./mathFieldEdits";

/**
 * What either visual field (MathLive's `mathField`, the Rust `rustField`)
 * does to the note around its maths: leave it, remove it, paste prose
 * beside it, tidy a block's blank lines as it opens. `target` is the maths
 * the field may write to (`writableSpan`), null when it can't.
 */

export type Direction = "forward" | "backward" | "upward" | "downward";

/** Back to the note, the caret just outside the maths on that side. A block
 *  at the very start or end of the note gets a line to land on. */
export function leaveMaths(view: EditorView, target: ActiveMath | null, dir: Direction) {
  if (!target) {
    view.focus();
    return;
  }
  const { doc } = view.state;
  const ahead = dir === "forward" || dir === "downward";
  let changes: ChangeSpec | undefined;
  let anchor: number;
  if (target.block) {
    const first = doc.lineAt(target.start);
    const last = doc.lineAt(target.end);
    if (ahead) {
      if (last.to < doc.length) anchor = last.to + 1;
      else {
        changes = { from: doc.length, insert: "\n" };
        anchor = doc.length + 1;
      }
    } else if (first.from > 0) anchor = first.from - 1;
    else {
      changes = { from: 0, insert: "\n" };
      anchor = 0;
    }
  } else {
    anchor = ahead ? target.end : target.start;
    if (dir === "upward" || dir === "downward") {
      const moved = view.moveVertically(EditorSelection.cursor(anchor), ahead).head;
      if (moved < target.start || moved > target.end) anchor = moved;
    }
  }
  view.dispatch({ changes, selection: { anchor }, scrollIntoView: true, userEvent: changes ? "input" : "select" });
  view.focus();
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
  view.focus();
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
  view.focus();
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
