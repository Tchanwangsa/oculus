import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

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
  const tree = syntaxTree(state);
  for (const side of [-1, 1] as const) {
    for (let n: SyntaxNode | null = tree.resolveInner(pos, side); n; n = n.parent) {
      if (n.name !== "InlineMath" && n.name !== "BlockMath") continue;
      const marks = n.getChildren("MathMark");
      if (marks.length < 2) return null;
      const from = marks[0].to;
      const to = marks[marks.length - 1].from;
      if (pos < from || pos > to) return null;
      return { node: n, start: n.from, end: n.to, from, to, display: n.name === "BlockMath" };
    }
  }
  return emptyPair(state, pos);
}

/** The caret between a lone `$$` pair — what Σ inserts mid-line — counts as
 *  empty inline maths, so the helpers work before the first character. */
function emptyPair(state: EditorState, pos: number): MathContext | null {
  const { doc } = state;
  const at = (i: number) => (i >= 0 && i < doc.length ? doc.sliceString(i, i + 1).charCodeAt(0) : -1);
  if (at(pos - 1) !== DOLLAR || at(pos) !== DOLLAR) return null;
  if (at(pos - 2) === DOLLAR || at(pos - 2) === BACKSLASH || at(pos + 1) === DOLLAR) return null;
  for (let n: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1); n; n = n.parent) {
    if (CODE.has(n.name)) return null;
  }
  return { node: null, start: pos - 1, end: pos + 1, from: pos, to: pos, display: false };
}
