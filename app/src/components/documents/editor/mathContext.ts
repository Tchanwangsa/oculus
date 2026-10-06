import type { EditorState } from "@codemirror/state";
import type { SyntaxNode, SyntaxNodeRef } from "@lezer/common";
import { ancestorAt } from "./syntax";

/** The maths around a position: its span, its LaTeX between the delimiters,
 *  and whether it is a display block. */
export interface MathContext {
  /** The parsed node; null for an empty `$$` pair, which never parses. */
  node: SyntaxNode | null;
  /** The whole maths, delimiters included. */
  start: number;
  end: number;
  /** Start and end of the LaTeX, delimiters excluded. */
  from: number;
  to: number;
  display: boolean;
}

const DOLLAR = 36;
const BACKSLASH = 92;
const CODE = new Set(["InlineCode", "FencedCode", "CodeBlock"]);

/** The `InlineMath` or `BlockMath` whose LaTeX holds `pos`, edges included;
 *  null on or outside the delimiters. */
export function mathAt(state: EditorState, pos: number): MathContext | null {
  const node = ancestorAt(state, pos, (n) => n.name === "InlineMath" || n.name === "BlockMath", [-1, 1]);
  if (!node) return emptyPair(state, pos);
  const ctx = mathContextOf(node);
  if (!ctx || pos < ctx.from || pos > ctx.to) return null;
  return ctx;
}

/** The span and LaTeX range of an `InlineMath` or `BlockMath` node. */
export function mathContextOf(node: SyntaxNode): MathContext | null {
  const marks = node.getChildren("MathMark");
  if (marks.length < 2) return null;
  const from = marks[0].to;
  const to = marks[marks.length - 1].from;
  return { node, start: node.from, end: node.to, from, to, display: node.name === "BlockMath" };
}

/** A block that starts its line (not inside a quote or list item, whose
 *  markers a block widget would swallow) and ends its last. */
export function ownsLines(state: EditorState, node: SyntaxNodeRef): boolean {
  const first = state.doc.lineAt(node.from);
  const last = state.doc.lineAt(node.to);
  return node.from === first.from && state.sliceDoc(node.to, last.to).trim() === "";
}

/** A line's container markup (quote markers, list markers, indent), as the
 *  prefix a new line inside the same container needs: list markers become
 *  spaces, quote markers stay. */
export function continuation(lineText: string): string {
  const markup = /^(?:[ \t]*(?:>[ \t]?|(?:[-*+]|\d{1,9}[.)])(?:[ \t]+|$)))*[ \t]*/.exec(lineText)![0];
  return markup.replace(/[-*+]|\d{1,9}[.)]/g, (m) => " ".repeat(m.length));
}

/** The caret between a lone `$$` pair mid-line — what a typed `$` and Σ
 *  insert — counts as empty inline maths, so the helpers work before the
 *  first character. */
function emptyPair(state: EditorState, pos: number): MathContext | null {
  const { doc } = state;
  const at = (i: number) => (i >= 0 && i < doc.length ? doc.sliceString(i, i + 1).charCodeAt(0) : -1);
  if (at(pos - 1) !== DOLLAR || at(pos) !== DOLLAR) return null;
  if (at(pos - 2) === DOLLAR || at(pos - 2) === BACKSLASH || at(pos + 1) === DOLLAR) return null;
  if (ancestorAt(state, pos, (node) => CODE.has(node.name), [1])) return null;
  return { node: null, start: pos - 1, end: pos + 1, from: pos, to: pos, display: false };
}
