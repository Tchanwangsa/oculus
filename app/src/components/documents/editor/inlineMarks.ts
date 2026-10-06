import { syntaxTree } from "@codemirror/language";
import { EditorSelection, type ChangeSpec, type EditorState, type SelectionRange } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

/**
 * Inline marks as data: how a kind of mark finds its delimiters in the syntax
 * tree, and how to take it off just the selected text. `commands.ts` picks the
 * kind; nothing here knows bold from link.
 */

interface Span {
  from: number;
  to: number;
}

/** The delimiters around inline text: `**` … `**`, or `[` … `](url)`. */
export interface Wrap {
  /** The node holding the delimiters and the text between them. */
  node: SyntaxNode;
  open: Span;
  close: Span;
}

export interface MarkKind {
  /** The syntax node that spans the whole mark. */
  node: string;
  wrap: (node: SyntaxNode) => Wrap | null;
  /** Whether the text just inside a delimiter must not be whitespace
   *  (emphasis cannot open or close on a space); a split moves it outside. */
  hugs: boolean;
}

/** A node whose first and last `markName` children are its delimiters. */
function delimited(node: string, markName: string, hugs: boolean): MarkKind {
  return {
    node,
    hugs,
    wrap: (n) => {
      const marks = n.getChildren(markName);
      if (marks.length < 2) return null;
      return { node: n, open: marks[0], close: marks[marks.length - 1] };
    },
  };
}

export const BOLD = delimited("StrongEmphasis", "EmphasisMark", true);
export const ITALIC = delimited("Emphasis", "EmphasisMark", true);
export const STRIKE = delimited("Strikethrough", "StrikethroughMark", true);
// Code keeps its spaces: they are part of what it shows.
export const CODE = delimited("InlineCode", "CodeMark", false);

/** `[` … `](url)`: the closer carries the destination, so a split link keeps
 *  pointing at it on both sides. */
export const LINK: MarkKind = {
  node: "Link",
  hugs: false,
  wrap: (n) => {
    const marks = n.getChildren("LinkMark");
    if (marks.length < 2) return null;
    return { node: n, open: { from: n.from, to: marks[0].to }, close: { from: marks[1].from, to: n.to } };
  },
};

/** `range` without the whitespace at its ends, which is never the text a mark
 *  is meant for: a selection of ` **a** ` is the bold span `a`. A selection of
 *  only whitespace is left alone. */
export function trimmed(state: EditorState, range: SelectionRange): SelectionRange {
  const text = state.sliceDoc(range.from, range.to);
  const from = range.from + (text.length - text.trimStart().length);
  const to = range.to - (text.length - text.trimEnd().length);
  return from < to ? EditorSelection.range(from, to) : range;
}

/**
 * The edits that take the mark off `range` alone. A caret, or a selection
 * covering all the text, drops the delimiters. Otherwise the mark is split
 * around the selection: each side keeps it only if it has text left, by
 * closing early (`**a **b` → `**a**`) or reopening late, and the selected
 * text is left bare. A split never lands inside a nested construct (an
 * italic, a link, code), which would cross delimiters: the bare text widens
 * to that construct's edge instead.
 */
export function unwrapWithin(state: EditorState, range: SelectionRange, wrap: Wrap, hugs: boolean): ChangeSpec[] {
  const inner = { from: wrap.open.to, to: wrap.close.from };
  let from = Math.max(range.from, inner.from);
  let to = Math.min(range.to, inner.to);
  if (range.empty || from >= to) return [wrap.open, wrap.close];
  for (let child = wrap.node.firstChild; child; child = child.nextSibling) {
    if (child.from < from && from < child.to) from = child.from;
    if (child.from < to && to < child.to) to = child.to;
  }

  const before = state.sliceDoc(inner.from, from);
  const after = state.sliceDoc(to, inner.to);
  const kept = { before: hugs ? before.trimEnd() : before, after: hugs ? after.trimStart() : after };

  // Closing at the end of the kept text leaves any spaces outside the mark.
  const head: ChangeSpec = kept.before
    ? { from: inner.from + kept.before.length, insert: state.sliceDoc(wrap.close.from, wrap.close.to) }
    : wrap.open;
  const tail: ChangeSpec = kept.after
    ? { from: inner.to - kept.after.length, insert: state.sliceDoc(wrap.open.from, wrap.open.to) }
    : wrap.close;
  return [head, tail];
}

/** Nodes whose children are inline text, where a mark can be laid out. */
const INLINE_HOSTS = /^(?:Paragraph|(?:ATX|Setext)Heading\d|TableCell|TableHeader|StrongEmphasis|Emphasis|Strikethrough|Link)$/;

/**
 * The edits that put the mark on all of `range`, which may overlap spans of
 * the same mark: an overlapped span merges in. One that sticks out of the
 * selection keeps its delimiter on that side and loses the one inside, and
 * one fully inside loses both, so `**ac*hi*e**` with `hi*e` selected becomes
 * `**ac*hie***`. Like `unwrapWithin`, an edge never lands inside a different
 * nested construct: it widens to that construct's edge. The selection is
 * trimmed first, since a mark cannot open or close on a space.
 */
export function wrapWithin(state: EditorState, range: SelectionRange, kind: MarkKind, marker: string): ChangeSpec[] {
  const core = trimmed(state, range);
  if (core.empty || /^\s*$/.test(state.sliceDoc(core.from, core.to))) return [];
  let { from, to } = core;

  let host = syntaxTree(state).resolveInner(from, 1);
  // Not a node of the mark itself: its children are text to lay the mark over.
  const hosts = (n: SyntaxNode) => n.name !== kind.node && INLINE_HOSTS.test(n.name);
  while (host.parent && !(host.from <= from && to <= host.to && hosts(host))) host = host.parent;
  if (!hosts(host)) return [{ from, insert: marker }, { from: to, insert: marker }];

  const edits: ChangeSpec[] = [];
  let opens = true;
  let closes = true;
  for (let child = host.firstChild; child; child = child.nextSibling) {
    if (child.to <= from || child.from >= to) continue;
    const wrap = child.name === kind.node ? kind.wrap(child) : null;
    if (!wrap) {
      from = Math.min(from, child.from);
      to = Math.max(to, child.to);
    } else if (child.from < from) {
      edits.push(wrap.close);
      opens = false;
    } else if (child.to > to) {
      edits.push(wrap.open);
      closes = false;
    } else {
      edits.push(wrap.open, wrap.close);
    }
  }
  if (opens) edits.push({ from, insert: marker });
  if (closes) edits.push({ from: to, insert: marker });
  return edits;
}
