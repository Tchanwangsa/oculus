import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";
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
  const marks = node.getChildren("MathMark");
  if (marks.length < 2) return null;
  const from = marks[0].to;
  const to = marks[marks.length - 1].from;
  if (pos < from || pos > to) return null;
  return { node, start: node.from, end: node.to, from, to, display: node.name === "BlockMath" };
}

/** The caret between a lone `$$` pair — what Σ inserts mid-line — counts as
 *  empty inline maths, so the helpers work before the first character. */
function emptyPair(state: EditorState, pos: number): MathContext | null {
  const { doc } = state;
  const at = (i: number) => (i >= 0 && i < doc.length ? doc.sliceString(i, i + 1).charCodeAt(0) : -1);
  if (at(pos - 1) !== DOLLAR || at(pos) !== DOLLAR) return null;
  if (at(pos - 2) === DOLLAR || at(pos - 2) === BACKSLASH || at(pos + 1) === DOLLAR) return null;
  if (ancestorAt(state, pos, (node) => CODE.has(node.name), [1])) return null;
  return { node: null, start: pos - 1, end: pos + 1, from: pos, to: pos, display: false };
}
