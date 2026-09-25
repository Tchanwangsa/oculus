/**
 * The keyboard niceties of the document editor, as pure functions over a
 * string and a selection — the part of a markdown editor that is worth
 * testing without a DOM.
 *
 * Each returns a `TextEdit`, a replacement of one span by another with the
 * caret's landing spot, or `null` to say "this key means nothing special
 * here, let the textarea have it". The component turns an edit into a real
 * insertion (`document.execCommand("insertText")`), which is why the shape is
 * a splice rather than a new value: an insertion the browser makes is one it
 * can undo, and a value swapped in from React is not.
 */

/** Replace `value.slice(start, end)` with `text`, then put the caret at
 *  `caret`. */
export interface TextEdit {
  start: number;
  end: number;
  text: string;
  caret: number;
}

/** Two spaces: what Tab writes, and what Shift+Tab takes back. */
export const INDENT = "  ";

/** A list marker at the start of a line — `- `, `* `, `+ `, `1. `, `1) ` —
 *  optionally carrying a task box (`- [ ] `, `- [x] `). Groups: indentation,
 *  the marker itself, the box. */
const LIST_LINE = /^([ \t]*)([-*+]|\d+[.)])[ \t]+(\[[ xX]\][ \t]+)?/;

/** Start of the line the caret is on. */
function lineStart(value: string, at: number): number {
  return value.lastIndexOf("\n", at - 1) + 1;
}

/** End of the line the caret is on (the index of its `\n`, or the end). */
function lineEnd(value: string, at: number): number {
  const i = value.indexOf("\n", at);
  return i === -1 ? value.length : i;
}

/** The pure form of a `TextEdit`, for tests and for the fallback path when the
 *  browser will not perform the insertion itself. */
export function applyEdit(value: string, edit: TextEdit): string {
  return value.slice(0, edit.start) + edit.text + value.slice(edit.end);
}

/**
 * Tab and Shift+Tab.
 *
 * With the caret on one line, Tab writes two spaces where the caret is —
 * the thing a Tab in a note is almost always for, a nested list item — and
 * Shift+Tab removes up to two leading spaces from that line. Across a
 * multi-line selection both act on every line touched, the way a code editor
 * does, so a block of list items nests and un-nests together.
 */
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
 * Enter at the end of a list line.
 *
 * Continues the list: `- ` gives another `- `, `3. ` gives `4. `, `- [x] `
 * gives an unticked `- [ ] `, each at the same indentation. Enter on an item
 * that is still empty — the marker and nothing after it — is the gesture for
 * "I am done with this list", so the marker is removed and the line left
 * blank rather than a second empty bullet being added.
 *
 * `null` when the caret is not at the end of a list line, or when there is a
 * selection: both are an ordinary Enter.
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
    // An empty item: drop the marker and stay on the line.
    return { start: from, end: to, text: "", caret: from };
  }

  const digits = /^(\d+)([.)])$/.exec(marker);
  const nextMarker = digits ? `${Number(digits[1]) + 1}${digits[2]}` : marker;
  const nextBox = box ? "[ ] " : "";
  const text = `\n${indent}${nextMarker} ${nextBox}`;
  return { start: selStart, end: selEnd, text, caret: selStart + text.length };
}

/**
 * A picture arriving at the caret.
 *
 * Markdown is happy to put an image inside a paragraph, and inline is exactly
 * what a pasted screenshot must not be: it lands on the baseline of whatever
 * was being typed, a full-width figure wedged into a sentence. So the tag is
 * given a line of its own — a blank line before it unless the caret already
 * has one, a break after it unless the text already breaks — which is what
 * the preview needs to draw it as a block, and what the student would have
 * typed by hand.
 *
 * The caret lands *after* the picture, on the line below, so writing carries
 * on under it.
 */
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
