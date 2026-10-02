import type {
  BlockContext,
  Element,
  InlineContext,
  Line,
  MarkdownConfig,
} from "@lezer/markdown";
import { tags } from "@lezer/highlight";

/**
 * Maths for the note parser, in the shapes `normalizeMath` and remark-math
 * accept: inline `$…$` (pandoc's rule — no space just inside either `$`, no
 * digit right after the closing one, so "$5 and $10" stays text) or `\(…\)`;
 * display `$$…$$` / `\[…\]` on lines of their own, one line or many. An
 * unclosed display block runs to the end of its container, as a fence does.
 *
 * Nodes: `InlineMath` and `BlockMath`, each holding two `MathMark` children;
 * the source is whatever lies between them.
 */

const DOLLAR = 36;
const BACKSLASH = 92;
const OPEN_PAREN = 40;
const CLOSE_PAREN = 41;
const OPEN_BRACKET = 91;
const NEWLINE = 10;

const isSpace = (c: number) => c === 32 || c === 9 || c === 10 || c === 13;
const isDigit = (c: number) => c >= 48 && c <= 57;

function dollarMath(cx: InlineContext, next: number, pos: number): number {
  if (next !== DOLLAR) return -1;
  // `$$` is display maths, which only counts on a line of its own.
  if (cx.char(pos + 1) === DOLLAR || cx.char(pos - 1) === DOLLAR) return -1;
  const first = cx.char(pos + 1);
  if (first < 0 || isSpace(first)) return -1;
  for (let i = pos + 1; i < cx.end; i++) {
    const c = cx.char(i);
    if (c === NEWLINE) return -1;
    if (c === BACKSLASH) {
      i++;
      continue;
    }
    if (c !== DOLLAR) continue;
    if (isSpace(cx.char(i - 1)) || isDigit(cx.char(i + 1))) continue;
    if (cx.char(i + 1) === DOLLAR) return -1;
    return cx.addElement(
      cx.elt("InlineMath", pos, i + 1, [
        cx.elt("MathMark", pos, pos + 1),
        cx.elt("MathMark", i, i + 1),
      ]),
    );
  }
  return -1;
}

/** `\(…\)` on one line. Runs before `Escape`, which would eat the `\(`. */
function parenMath(cx: InlineContext, next: number, pos: number): number {
  if (next !== BACKSLASH || cx.char(pos + 1) !== OPEN_PAREN) return -1;
  for (let i = pos + 2; i < cx.end - 1; i++) {
    const c = cx.char(i);
    if (c === NEWLINE) return -1;
    if (c === BACKSLASH && cx.char(i + 1) === CLOSE_PAREN) {
      if (i === pos + 2) return -1;
      return cx.addElement(
        cx.elt("InlineMath", pos, i + 2, [
          cx.elt("MathMark", pos, pos + 2),
          cx.elt("MathMark", i, i + 2),
        ]),
      );
    }
  }
  return -1;
}

/** The opening delimiter at the line's content start, or null. */
function openDelimiter(line: Line): { open: string; close: string } | null {
  const at = line.pos;
  if (line.indent - line.baseIndent >= 4) return null;
  if (line.next === DOLLAR && line.text.charCodeAt(at + 1) === DOLLAR) {
    return { open: "$$", close: "$$" };
  }
  if (line.next === BACKSLASH && line.text.charCodeAt(at + 1) === OPEN_BRACKET) {
    return { open: "\\[", close: "\\]" };
  }
  return null;
}

function blockMath(cx: BlockContext, line: Line): boolean {
  const delim = openDelimiter(line);
  if (!delim) return false;
  const from = cx.lineStart + line.pos;
  const marks: Element[] = [cx.elt("MathMark", from, from + 2)];

  // One line: `$$ x $$`, with nothing after the closer.
  const rest = line.text.slice(line.pos + 2);
  const closeAt = rest.lastIndexOf(delim.close);
  if (closeAt >= 0 && rest.slice(closeAt + 2).trim() === "") {
    const at = from + 2 + closeAt;
    marks.push(cx.elt("MathMark", at, at + 2));
    cx.nextLine();
    cx.addElement(cx.elt("BlockMath", from, at + 2, marks));
    return true;
  }

  // Many lines: the first line ending in the closer ends the block.
  // `line.depth` is runtime-only; below the stack the line left our container.
  let to = cx.lineStart + line.text.length;
  while (cx.nextLine() && (line as unknown as { depth: number }).depth >= cx.depth) {
    for (const m of line.markers) marks.push(m);
    const text = line.text.trimEnd();
    to = cx.lineStart + line.text.length;
    if (text.endsWith(delim.close) && text.length - 2 >= line.pos) {
      const at = cx.lineStart + text.length - 2;
      marks.push(cx.elt("MathMark", at, at + 2));
      to = at + 2;
      cx.nextLine();
      break;
    }
  }
  cx.addElement(cx.elt("BlockMath", from, to, marks));
  return true;
}

export const MathSyntax: MarkdownConfig = {
  defineNodes: [
    { name: "InlineMath", style: tags.special(tags.content) },
    { name: "BlockMath", block: true, style: tags.special(tags.content) },
    { name: "MathMark", style: tags.processingInstruction },
  ],
  parseInline: [
    { name: "InlineMath", parse: dollarMath },
    { name: "ParenMath", parse: parenMath, before: "Escape" },
  ],
  parseBlock: [
    {
      name: "BlockMath",
      parse: blockMath,
      // A display block may follow a paragraph line directly.
      endLeaf: (_cx, line) => openDelimiter(line) !== null,
      before: "FencedCode",
    },
  ],
};
