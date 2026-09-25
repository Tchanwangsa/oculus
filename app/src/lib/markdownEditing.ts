/**
 * The document editor's key handling as pure functions. Each returns a
 * `TextEdit` or `null` (let the textarea have the key). It is a splice, not a
 * new value, because the component applies it with `execCommand("insertText")`
 * — an insertion the browser makes is undoable; a React value swap is not.
 */

/** Replace `value.slice(start, end)` with `text`, caret at `caret`. */
export interface TextEdit {
  start: number;
  end: number;
  text: string;
  caret: number;
}

export const INDENT = "  ";

/** A list-line prefix, optionally with a task box. Groups: indent, marker, box. */
const LIST_LINE = /^([ \t]*)([-*+]|\d+[.)])[ \t]+(\[[ xX]\][ \t]+)?/;

function lineStart(value: string, at: number): number {
  return value.lastIndexOf("\n", at - 1) + 1;
}

function lineEnd(value: string, at: number): number {
  const i = value.indexOf("\n", at);
  return i === -1 ? value.length : i;
}

/** For tests, and the fallback when the browser will not insert. */
export function applyEdit(value: string, edit: TextEdit): string {
  return value.slice(0, edit.start) + edit.text + value.slice(edit.end);
}

/** Tab inserts INDENT at the caret; Shift+Tab strips up to two leading spaces.
 *  A multi-line selection indents or outdents every line touched. */
export function tabEdit(
  value: string,
  selStart: number,
  selEnd: number,
  outdent: boolean,
): TextEdit {
  const multiline = value.slice(selStart, selEnd).includes("\n");
  if (!outdent && !multiline) {
    return { start: selStart, end: selEnd, text: INDENT, caret: selStart + INDENT.length };
  }

  const from = lineStart(value, selStart);
  const to = lineEnd(value, Math.max(selStart, selEnd - 1));
  const lines = value.slice(from, to).split("\n");
  let caretShift = 0;
  const edited = lines.map((line, i) => {
    if (outdent) {
      const removed = line.startsWith(INDENT) ? INDENT.length : line.startsWith(" ") ? 1 : 0;
      if (i === 0) caretShift = -Math.min(removed, selStart - from);
      return line.slice(removed);
    }
    if (i === 0) caretShift = INDENT.length;
    return INDENT + line;
  });
  const text = edited.join("\n");
  return { start: from, end: to, text, caret: Math.max(from, selStart + caretShift) };
}

/**
 * Enter at the end of a list line continues the list (`3. ` → `4. `, a ticked
 * box → `[ ] `); on an empty item it removes the marker to end the list.
 * `null` for a selection or a caret not at the end of a list line.
 */
export function enterEdit(value: string, selStart: number, selEnd: number): TextEdit | null {
  if (selStart !== selEnd) return null;
  const from = lineStart(value, selStart);
  const to = lineEnd(value, selStart);
  if (selStart !== to) return null;
  const line = value.slice(from, to);
  const m = LIST_LINE.exec(line);
  if (!m) return null;

  const [prefix, indent, marker, box] = m;
  if (line.length === prefix.length) {
    return { start: from, end: to, text: "", caret: from };
  }

  const digits = /^(\d+)([.)])$/.exec(marker);
  const nextMarker = digits ? `${Number(digits[1]) + 1}${digits[2]}` : marker;
  const nextBox = box ? "[ ] " : "";
  const text = `\n${indent}${nextMarker} ${nextBox}`;
  return { start: selStart, end: selEnd, text, caret: selStart + text.length };
}

/** Insert an image as its own block (blank lines around it, so it is not drawn
 *  inline), caret after it. */
export function imageEdit(
  value: string,
  selStart: number,
  selEnd: number,
  markdown: string,
): TextEdit {
  const before = value.slice(0, selStart);
  const after = value.slice(selEnd);
  const prefix =
    before === "" || before.endsWith("\n\n") ? ""
    : before.endsWith("\n") ? "\n"
    : "\n\n";
  const suffix = after.startsWith("\n\n") ? "" : after.startsWith("\n") ? "\n" : "\n\n";
  const text = `${prefix}${markdown}${suffix}`;
  return { start: selStart, end: selEnd, text, caret: selStart + text.length };
}
