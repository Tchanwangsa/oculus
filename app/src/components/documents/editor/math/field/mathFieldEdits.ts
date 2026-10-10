import { Annotation, EditorSelection, type EditorState, type Text } from "@codemirror/state";

/**
 * How the visual maths field (`field/rustField`) writes into the note. The
 * note's history is the only undo: the field's writes are ordinary typing
 * transactions, so consecutive keystrokes join one undo step, and a field
 * only ever writes to the maths it was mounted on.
 */

/** User event of a field write: under `input.type`, which CodeMirror's
 *  history joins with the keystrokes before it. */
export const FIELD_INPUT = "input.type.math";

/** Marks a transaction as a field's own write, carrying the field's id. */
export const fieldWrite = Annotation.define<number>();

/** The part of the maths a field may write to. */
export interface FieldSpan {
  /** Identity of the maths, kept while it is mapped through edits. */
  id: number;
  start: number;
  end: number;
  from: number;
  to: number;
  block: boolean;
}

/** `active` when it is the maths the field was mounted on (`id`) and its
 *  LaTeX is still what the field last wrote or loaded (`shown`); else null,
 *  and the field drops its write rather than overwrite something else. */
export function writableSpan<T extends FieldSpan>(state: EditorState, active: T | null, id: number, shown: string): T | null {
  if (!active || active.id !== id) return null;
  return state.sliceDoc(active.from, active.to).trim() === shown ? active : null;
}

const isHigh = (c: number) => c >= 0xd800 && c <= 0xdbff;
const isLow = (c: number) => c >= 0xdc00 && c <= 0xdfff;

/** The smallest change turning `before` (at `at` in the doc) into `after`,
 *  or null when they match. Small changes keep history steps adjacent, so a
 *  run of keystrokes joins one step. */
export function minimalChange(before: string, after: string, at: number): { from: number; to: number; insert: string } | null {
  if (before === after) return null;
  const max = Math.min(before.length, after.length);
  let p = 0;
  while (p < max && before.charCodeAt(p) === after.charCodeAt(p)) p++;
  if (p > 0 && isHigh(before.charCodeAt(p - 1))) p--;
  let s = 0;
  while (s < max - p && before.charCodeAt(before.length - 1 - s) === after.charCodeAt(after.length - 1 - s)) s++;
  if (s > 0 && isLow(before.charCodeAt(before.length - s))) s--;
  return { from: at + p, to: at + before.length - s, insert: after.slice(p, after.length - s) };
}

/** Where the field's maths starts and ends in the note: a block's whole
 *  lines, which its widget covers. */
export function fieldEdges(doc: Text, span: Pick<FieldSpan, "start" | "end" | "block">): { first: number; last: number } {
  return span.block
    ? { first: doc.lineAt(span.start).from, last: doc.lineAt(span.end).to }
    : { first: span.start, last: span.end };
}

/**
 * The note's selection sits inside the field's LaTeX while the field is
 * open, out of sight. A selection extended from there (Shift-click, say) to
 * past the maths starts at the maths' edge instead, so it holds exactly the
 * text that is highlighted. Null when nothing needs moving.
 */
export function selectionPastField(
  doc: Text,
  span: Pick<FieldSpan, "start" | "end" | "block">,
  sel: EditorSelection,
): EditorSelection | null {
  if (sel.ranges.length !== 1) return null;
  const { anchor, head } = sel.main;
  const { first, last } = fieldEdges(doc, span);
  if (anchor <= first || anchor >= last || (head >= first && head <= last)) return null;
  return EditorSelection.single(head > last ? last : first, head);
}

/** `latex` without blank lines at either end and never two in a row: they
 *  would end the block's paragraph, and the field's empty rows (`\\` lines)
 *  need none. */
export function squeezeBlankLines(latex: string): string {
  const out: string[] = [];
  for (const line of latex.split("\n")) {
    if (line.trim() || (out.length && out[out.length - 1].trim())) out.push(line);
  }
  while (out.length && !out[out.length - 1].trim()) out.pop();
  return out.join("\n");
}

/** A block's LaTeX without empty rows at its end (`\\` lines past the last
 *  formula, what Shift+Enter on the last row leaves), its trailing whitespace kept;
 *  unchanged when nothing else would be left. */
export function withoutEndRows(latex: string): string {
  const body = latex.trimEnd();
  const kept = body.replace(/(?:\s*\\\\(?:\[[^\]]*\])?)+$/, "");
  return kept === body || !kept.trim() ? latex : kept + latex.slice(body.length);
}
