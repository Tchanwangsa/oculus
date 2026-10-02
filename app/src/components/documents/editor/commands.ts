import { indentLess, indentMore } from "@codemirror/commands";
import { syntaxTree } from "@codemirror/language";
import { deleteMarkupBackward, insertNewlineContinueMarkup } from "@codemirror/lang-markdown";
import {
  EditorSelection,
  type ChangeSpec,
  type EditorState,
  type SelectionRange,
  type StateCommand,
  type Transaction,
} from "@codemirror/state";
import { EditorView, type Command, type KeyBinding } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";

/**
 * Editing commands for notes: plain CodeMirror commands that rewrite markdown
 * text, shared by the keymap and the toolbar. Every toggle unwraps when the
 * selection is already formatted that way.
 */

import { ancestorAt } from "./syntax";

type Dispatch = (tr: Transaction) => void;

/** The innermost `name` node holding the range. A caret must be strictly
 *  inside, so one just past `**bold**` starts new bold rather than unwrapping. */
function enclosing(state: EditorState, range: SelectionRange, name: string): SyntaxNode | null {
  return ancestorAt(state, range.from, (node) =>
    node.name === name && (range.empty
      ? node.from < range.from && range.to < node.to
      : node.from <= range.from && range.to <= node.to),
  );
}

// ── Inline marks ──────────────────────────────────────────────────────────

function toggleInline(marker: string, nodeName: string, markName: string): StateCommand {
  return ({ state, dispatch }) => {
    const tr = state.changeByRange((range) => {
      const node = enclosing(state, range, nodeName);
      if (node) {
        const marks = node.getChildren(markName);
        if (marks.length >= 2) {
          const open = marks[0];
          const close = marks[marks.length - 1];
          const changes = state.changes([
            { from: open.from, to: open.to },
            { from: close.from, to: close.to },
          ]);
          return { changes, range: range.map(changes) };
        }
      }
      if (range.empty) {
        return {
          changes: { from: range.from, insert: marker + marker },
          range: EditorSelection.cursor(range.from + marker.length),
        };
      }
      // Emphasis cannot open or close on a space, so the marks hug the text.
      const text = state.sliceDoc(range.from, range.to);
      const from = range.from + (text.length - text.trimStart().length);
      const to = range.to - (text.length - text.trimEnd().length);
      if (from >= to) return { range };
      return {
        changes: [
          { from, insert: marker },
          { from: to, insert: marker },
        ],
        range: EditorSelection.range(from + marker.length, to + marker.length),
      };
    });
    dispatch(state.update(tr, { scrollIntoView: true, userEvent: "input" }));
    return true;
  };
}

export const toggleBold = toggleInline("**", "StrongEmphasis", "EmphasisMark");
export const toggleItalic = toggleInline("*", "Emphasis", "EmphasisMark");
export const toggleStrike = toggleInline("~~", "Strikethrough", "StrikethroughMark");
export const toggleCode = toggleInline("`", "InlineCode", "CodeMark");

/** `[sel](|)`, caret in the URL; an empty selection gets `[|]()`, a selected
 *  URL `[|](url)`. Inside a link, unwraps it to its text. */
export const toggleLink: StateCommand = ({ state, dispatch }) => {
  const tr = state.changeByRange((range) => {
    const link = enclosing(state, range, "Link");
    const marks = link?.getChildren("LinkMark") ?? [];
    if (link && marks.length >= 2) {
      const changes = state.changes([
        { from: link.from, to: marks[0].to },
        { from: marks[1].from, to: link.to },
      ]);
      return { changes, range: range.map(changes) };
    }
    const text = state.sliceDoc(range.from, range.to);
    if (/^(https?:\/\/|www\.)\S+$/i.test(text)) {
      return {
        changes: { from: range.from, to: range.to, insert: `[](${text})` },
        range: EditorSelection.cursor(range.from + 1),
      };
    }
    const insert = `[${text}]()`;
    return {
      changes: { from: range.from, to: range.to, insert },
      range: EditorSelection.cursor(range.from + (text ? insert.length - 1 : 1)),
    };
  });
  dispatch(state.update(tr, { scrollIntoView: true, userEvent: "input" }));
  return true;
};

// ── Line prefixes ─────────────────────────────────────────────────────────

/** Line numbers the selection covers; a range ending at a line's start
 *  does not take that line. */
function selectedLines(state: EditorState): number[] {
  const lines = new Set<number>();
  for (const r of state.selection.ranges) {
    const first = state.doc.lineAt(r.from).number;
    let last = state.doc.lineAt(r.to).number;
    if (!r.empty && last > first && state.doc.line(last).from === r.to) last--;
    for (let n = first; n <= last; n++) lines.add(n);
  }
  return [...lines].sort((a, b) => a - b);
}

/** Apply line edits, carets pushed past text inserted at them. */
function editLines(
  state: EditorState,
  dispatch: Dispatch,
  edit: (text: string, index: number) => { strip: number; insert: string } | null,
  at: (text: string) => number = (text) => text.length - text.trimStart().length,
): boolean {
  const specs: ChangeSpec[] = [];
  selectedLines(state).forEach((n, i) => {
    const ln = state.doc.line(n);
    const e = edit(ln.text, i);
    if (!e) return;
    const from = ln.from + at(ln.text);
    specs.push({ from, to: from + e.strip, insert: e.insert });
  });
  if (!specs.length) return false;
  const changes = state.changes(specs);
  const selection = EditorSelection.create(
    state.selection.ranges.map((r) =>
      EditorSelection.range(changes.mapPos(r.anchor, 1), changes.mapPos(r.head, 1)),
    ),
    state.selection.mainIndex,
  );
  dispatch(state.update({ changes, selection, scrollIntoView: true, userEvent: "input" }));
  return true;
}

const HEADING_PREFIX = /^#{1,6}[ \t]+|^#{1,6}$/;
/** A list or task prefix after indentation. */
const LIST_PREFIX = /^(?:[-*+]|\d+[.)])[ \t]+(?:\[[ xX]\][ \t]+)?/;

export type BlockType = "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6";

/** Make each selected line plain text or a heading. */
export function setBlockType(type: BlockType): StateCommand {
  const level = type === "p" ? 0 : Number(type.slice(1));
  return ({ state, dispatch }) =>
    editLines(state, dispatch, (text) => {
      const body = text.trimStart();
      const strip = HEADING_PREFIX.exec(body)?.[0].length ?? 0;
      const insert = level ? `${"#".repeat(level)} ` : "";
      if (!strip && !insert) return null;
      return { strip, insert };
    });
}

export type ListKind = "bullet" | "ordered" | "task" | "quote";

const HAS: Record<ListKind, RegExp> = {
  bullet: /^[-*+][ \t]+(?!\[[ xX]\][ \t])/,
  ordered: /^\d+[.)][ \t]+/,
  task: /^[-*+][ \t]+\[[ xX]\][ \t]+/,
  quote: /^>[ \t]?/,
};

/** Toggle a list kind or a quote on the selected lines: off if every
 *  non-blank line has it, else on, replacing any other list marker. */
export function toggleList(kind: ListKind): StateCommand {
  return ({ state, dispatch }) => {
    const lines = selectedLines(state).map((n) => state.doc.line(n).text);
    const filled = lines.filter((t) => t.trim() !== "");
    const on = filled.length > 0 && filled.every((t) => HAS[kind].test(t.trimStart()));
    const skipBlank = lines.length > 1;
    let count = 0;
    // A new quote marker goes at the line's start, a list marker after its indent.
    const at = kind === "quote" && !on ? () => 0 : undefined;
    return editLines(state, dispatch, (text) => {
      const body = text.trimStart();
      if (skipBlank && body === "") return null;
      if (kind === "quote") {
        if (on) return { strip: HAS.quote.exec(body)?.[0].length ?? 0, insert: "" };
        return { strip: 0, insert: "> " };
      }
      const existing = HAS[kind].exec(body)?.[0].length ?? 0;
      if (on) return { strip: existing, insert: "" };
      const strip = LIST_PREFIX.exec(body)?.[0].length ?? HEADING_PREFIX.exec(body)?.[0].length ?? 0;
      count += 1;
      const insert = kind === "bullet" ? "- " : kind === "task" ? "- [ ] " : `${count}. `;
      return { strip, insert };
    }, at);
  };
}

// ── Blocks ────────────────────────────────────────────────────────────────

/**
 * Insert `text` as a block of its own at the main selection, blank lines
 * around it (so a picture is not drawn inline and `---` is not read as a
 * heading underline). `caret` is an offset into `text`, or a range in it;
 * by default the caret lands after the block.
 */
function insertBlock(
  state: EditorState,
  dispatch: Dispatch,
  text: string,
  caret?: number | [number, number],
): boolean {
  const { from, to } = state.selection.main;
  const before = state.sliceDoc(0, from);
  const after = state.sliceDoc(to);
  const prefix = before === "" || before.endsWith("\n\n") ? "" : before.endsWith("\n") ? "\n" : "\n\n";
  const suffix = after.startsWith("\n\n") ? "" : after.startsWith("\n") ? "\n" : "\n\n";
  const insert = `${prefix}${text}${suffix}`;
  const start = from + prefix.length;
  const selection =
    caret === undefined ? EditorSelection.cursor(from + insert.length)
    : typeof caret === "number" ? EditorSelection.cursor(start + caret)
    : EditorSelection.range(start + caret[0], start + caret[1]);
  dispatch(
    state.update({
      changes: { from, to, insert },
      selection,
      scrollIntoView: true,
      userEvent: "input",
    }),
  );
  return true;
}

/** A picture's markdown, own block, caret after it. */
export function insertImage(markdown: string): StateCommand {
  return ({ state, dispatch }) => insertBlock(state, dispatch, markdown);
}

export const insertDivider: StateCommand = ({ state, dispatch }) => insertBlock(state, dispatch, "---");

/** A GFM table, `rows` counting the header, caret after it so Live mode
 *  draws the grid at once. */
export function insertTable(rows: number, cols: number): StateCommand {
  return ({ state, dispatch }) => {
    const header = `| ${Array.from({ length: cols }, (_, i) => `Column ${i + 1}`).join(" | ")} |`;
    const rule = `|${" --- |".repeat(cols)}`;
    const body = Array.from({ length: Math.max(rows - 1, 1) }, () => `|${"   |".repeat(cols)}`);
    const text = [header, rule, ...body].join("\n");
    return insertBlock(state, dispatch, text);
  };
}

/** Wrap the selected lines in a fence, or open an empty one; inside a fence,
 *  remove it. */
export const toggleCodeBlock: StateCommand = ({ state, dispatch }) => {
  const main = state.selection.main;
  const fence = enclosingBlock(state, main.head, "FencedCode");
  if (fence) {
    const open = state.doc.lineAt(fence.from);
    const close = state.doc.lineAt(fence.to);
    const marks = fence.getChildren("CodeMark");
    const closed = marks.length > 1 && close.number > open.number;
    const changes: ChangeSpec[] = [{ from: open.from, to: Math.min(open.to + 1, state.doc.length) }];
    if (closed) {
      // The closing line with its newline; the last line takes the one before.
      const atEnd = close.to === state.doc.length && close.from - 1 > open.to;
      changes.push({ from: atEnd ? close.from - 1 : close.from, to: Math.min(close.to + 1, state.doc.length) });
    }
    dispatch(state.update({ changes, scrollIntoView: true, userEvent: "input" }));
    return true;
  }
  const first = state.doc.lineAt(main.from);
  const last = state.doc.lineAt(main.to);
  if (main.empty && first.text.trim() === "") {
    return insertBlock(state, dispatch, "```\n\n```", 4);
  }
  dispatch(
    state.update({
      changes: [
        { from: first.from, insert: "```\n" },
        { from: last.to, insert: "\n```" },
      ],
      scrollIntoView: true,
      userEvent: "input",
    }),
  );
  return true;
};

/** `$|$` inline, or a `$$` block on an empty line; inside inline maths,
 *  removes its delimiters. */
export const insertMath: StateCommand = ({ state, dispatch }) => {
  const main = state.selection.main;
  const inline = enclosing(state, main, "InlineMath");
  if (inline) {
    const marks = inline.getChildren("MathMark");
    if (marks.length >= 2) {
      const changes = state.changes([
        { from: marks[0].from, to: marks[0].to },
        { from: marks[marks.length - 1].from, to: marks[marks.length - 1].to },
      ]);
      dispatch(state.update({ changes, selection: main.map(changes), userEvent: "input" }));
      return true;
    }
  }
  const ln = state.doc.lineAt(main.from);
  if (main.empty && ln.text.trim() === "") {
    return insertBlock(state, dispatch, "$$\n\n$$", 3);
  }
  const text = state.sliceDoc(main.from, main.to);
  dispatch(
    state.update({
      changes: { from: main.from, to: main.to, insert: `$${text}$` },
      selection: text
        ? EditorSelection.range(main.from + 1, main.to + 1)
        : EditorSelection.cursor(main.from + 1),
      scrollIntoView: true,
      userEvent: "input",
    }),
  );
  return true;
};

// ── Tab ───────────────────────────────────────────────────────────────────

/** A list item's line, by its text. */
const LIST_LINE = /^[ \t]*(?:[-*+]|\d+[.)])[ \t]/;

/** Tab indents a list item, or every line of a multi-line selection, by two
 *  spaces; elsewhere it types two spaces. */
export const indentOrTab: Command = (view) => {
  const { state } = view;
  const multiline = state.selection.ranges.some(
    (r) => state.doc.lineAt(r.from).number !== state.doc.lineAt(r.to).number,
  );
  if (multiline || LIST_LINE.test(state.doc.lineAt(state.selection.main.head).text)) {
    return indentMore(view);
  }
  view.dispatch(state.replaceSelection("  "), { scrollIntoView: true, userEvent: "input" });
  return true;
};

/**
 * ⌘K for a focused note: the menu bar takes ⌘K for Search before the editor's
 * keymap sees it, so the palette's menu handler asks here first.
 */
export function linkInFocusedNote(): boolean {
  const dom = document.activeElement?.closest<HTMLElement>(".cm-editor");
  const view = dom ? EditorView.findFromDOM(dom) : null;
  return view ? toggleLink(view) : false;
}

export const noteKeymap: KeyBinding[] = [
  { key: "Mod-b", run: toggleBold },
  { key: "Mod-i", run: toggleItalic },
  { key: "Mod-Shift-x", run: toggleStrike },
  { key: "Mod-e", run: toggleCode },
  { key: "Mod-k", run: toggleLink },
  { key: "Tab", run: indentOrTab, shift: indentLess },
  { key: "Enter", run: insertNewlineContinueMarkup },
  { key: "Backspace", run: deleteMarkupBackward },
];

// ── Toolbar state ─────────────────────────────────────────────────────────

/** The innermost `name` block on the line holding `pos`. */
function enclosingBlock(state: EditorState, pos: number, name: string): SyntaxNode | null {
  const ln = state.doc.lineAt(pos);
  const start = ln.from + (ln.text.length - ln.text.trimStart().length);
  return ancestorAt(state, start, (node) => node.name === name, [1]);
}

export interface ActiveFormats {
  block: BlockType;
  bold: boolean;
  italic: boolean;
  strike: boolean;
  code: boolean;
  link: boolean;
  bullet: boolean;
  ordered: boolean;
  task: boolean;
  quote: boolean;
  codeBlock: boolean;
  math: boolean;
}

export const NO_FORMATS: ActiveFormats = {
  block: "p",
  bold: false,
  italic: false,
  strike: false,
  code: false,
  link: false,
  bullet: false,
  ordered: false,
  task: false,
  quote: false,
  codeBlock: false,
  math: false,
};

/** What the main selection sits in, from the syntax tree. */
export function activeFormats(state: EditorState): ActiveFormats {
  const main = state.selection.main;
  const ln = state.doc.lineAt(main.head);
  const start = ln.from + (ln.text.length - ln.text.trimStart().length);
  const blocks = new Set<string>();
  let item: SyntaxNode | null = null;
  for (let n: SyntaxNode | null = syntaxTree(state).resolveInner(start, 1); n; n = n.parent) {
    blocks.add(n.name);
    if (n.name === "ListItem" && !item) item = n;
  }
  const heading = [...blocks].map((b) => /^(?:ATX|Setext)Heading(\d)$/.exec(b)?.[1]).find(Boolean);
  const list = item?.parent?.name;
  const task = list === "BulletList" && item?.getChild("Task") != null;
  return {
    block: heading ? (`h${heading}` as BlockType) : "p",
    bold: enclosing(state, main, "StrongEmphasis") != null,
    italic: enclosing(state, main, "Emphasis") != null,
    strike: enclosing(state, main, "Strikethrough") != null,
    code: enclosing(state, main, "InlineCode") != null,
    link: enclosing(state, main, "Link") != null,
    bullet: list === "BulletList" && !task,
    ordered: list === "OrderedList",
    task,
    quote: blocks.has("Blockquote"),
    codeBlock: blocks.has("FencedCode"),
    math: blocks.has("BlockMath") || enclosing(state, main, "InlineMath") != null,
  };
}

export function sameFormats(a: ActiveFormats, b: ActiveFormats): boolean {
  return (Object.keys(a) as (keyof ActiveFormats)[]).every((k) => a[k] === b[k]);
}
