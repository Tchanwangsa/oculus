import { syntaxTree } from "@codemirror/language";
import {
  EditorSelection,
  EditorState,
  Prec,
  StateEffect,
  StateField,
  type ChangeSpec,
  type Extension,
  type Range,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  keymap,
  type Command,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import type { SyntaxNode, SyntaxNodeRef } from "@lezer/common";

import { fenceCode, fenceLanguage } from "./codeLanguages";
import { noteHost } from "./host";
import { enterTable, parseTable, TableWidget } from "./table";
import {
  BulletWidget,
  CheckboxWidget,
  CodeHeaderWidget,
  ImageWidget,
  MathWidget,
  MermaidWidget,
  PropertiesWidget,
  RuleWidget,
} from "./widgets";

/**
 * Live mode: markdown renders in place and a construct shows its source while
 * the selection touches it — headings, quotes, list markers and code fences
 * while the caret is on their lines. Unfocused, nothing is revealed. A table
 * never is: it is atomic, and its cells take the caret instead.
 *
 * Inline decorations come from a view plugin over the viewport; block widgets
 * (display maths, a picture alone on its line, a mermaid diagram, a rule, a
 * table, frontmatter properties) from a state field,
 * because CodeMirror rejects block decorations from a plugin. Both rebuild on
 * selection and focus changes, not only edits.
 */

const setFocused = StateEffect.define<boolean>();

const focusedField = StateField.define<boolean>({
  create: () => false,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setFocused)) value = e.value;
    return value;
  },
});

const trackFocus = EditorView.focusChangeEffect.of((_state, focusing) => setFocused.of(focusing));

/** Whether a selection range touches `from..to`, edges included. */
function touches(state: EditorState, from: number, to: number): boolean {
  if (!state.field(focusedField)) return false;
  return state.selection.ranges.some((r) => r.from <= to && r.to >= from);
}

/** The same, widened to whole lines. */
function touchesLines(state: EditorState, from: number, to: number): boolean {
  return touches(state, state.doc.lineAt(from).from, state.doc.lineAt(to).to);
}

const hide = Decoration.replace({});
const mark = (cls: string) => Decoration.mark({ class: cls });
const line = (cls: string) => Decoration.line({ class: cls });

/** `![alt](src)` pieces, or null for a reference image. */
function imageParts(state: EditorState, node: SyntaxNode) {
  const url = node.getChild("URL");
  const marks = node.getChildren("LinkMark");
  if (!url || marks.length < 2) return null;
  return {
    src: state.sliceDoc(url.from, url.to),
    alt: state.sliceDoc(marks[0].to, marks[1].from),
  };
}

/** A picture with nothing else on its line, drawn as a block. */
function isBlockImage(state: EditorState, node: SyntaxNodeRef): boolean {
  const ln = state.doc.lineAt(node.from);
  return node.to <= ln.to && ln.text.trim() === state.sliceDoc(node.from, node.to);
}

/** A block that starts its line (not inside a quote or list item, whose
 *  markers a block widget would swallow) and ends its last. */
function ownsLines(state: EditorState, node: SyntaxNodeRef): boolean {
  const first = state.doc.lineAt(node.from);
  const last = state.doc.lineAt(node.to);
  return node.from === first.from && state.sliceDoc(node.to, last.to).trim() === "";
}

/** The maths between a node's two `MathMark`s, and where the caret goes. */
function mathParts(state: EditorState, node: SyntaxNode) {
  const marks = node.getChildren("MathMark");
  if (marks.length < 2) return null;
  const from = marks[0].to;
  const to = marks[marks.length - 1].from;
  const source = state.sliceDoc(from, to);
  // Into the first content line, past a bare `$$` line.
  const newline = source.indexOf("\n");
  const caret = from - node.from + (newline >= 0 && source.slice(0, newline).trim() === "" ? newline + 1 : 0);
  return { source: source.trim(), caret };
}

/** A ```mermaid fence's source, or null for any other fence. */
function mermaidCode(state: EditorState, node: SyntaxNode): string | null {
  const info = node.getChild("CodeInfo");
  if (!info || state.sliceDoc(info.from, info.to).split(/\s/)[0].toLowerCase() !== "mermaid") return null;
  return fenceCode(node, (a, b) => state.sliceDoc(a, b)).trimEnd();
}

/** A mermaid fence the block field draws (in place of its source, or under it
 *  while the selection touches it). */
function isDrawnMermaid(state: EditorState, node: SyntaxNodeRef): boolean {
  return ownsLines(state, node) && mermaidCode(state, node.node) != null;
}

// ── Block decorations (state field) ────────────────────────────────────────

function buildBlocks(state: EditorState): DecorationSet {
  const host = state.facet(noteHost);
  const out: Range<Decoration>[] = [];
  syntaxTree(state).iterate({
    enter: (node) => {
      switch (node.name) {
        case "BlockMath": {
          if (!ownsLines(state, node) || touches(state, node.from, node.to)) return false;
          const parts = mathParts(state, node.node);
          if (!parts) return false;
          const to = state.doc.lineAt(node.to).to;
          out.push(
            Decoration.replace({ widget: new MathWidget(parts.source, true, parts.caret), block: true }).range(
              node.from,
              to,
            ),
          );
          return false;
        }
        case "HorizontalRule": {
          if (!ownsLines(state, node) || touchesLines(state, node.from, node.to)) return false;
          out.push(
            Decoration.replace({ widget: new RuleWidget(), block: true }).range(
              node.from,
              state.doc.lineAt(node.to).to,
            ),
          );
          return false;
        }
        case "Table": {
          // Never revealed: its cells are where it is edited (`tableKeys`).
          if (!ownsLines(state, node)) return false;
          const to = state.doc.lineAt(node.to).to;
          const source = state.sliceDoc(node.from, to);
          const layout = parseTable(source);
          if (layout) {
            out.push(Decoration.replace({ widget: new TableWidget(source, layout), block: true }).range(node.from, to));
          }
          return false;
        }
        case "Frontmatter": {
          const to = state.doc.lineAt(node.to).to;
          if (touches(state, node.from, to)) return false;
          out.push(
            Decoration.replace({ widget: new PropertiesWidget(state.sliceDoc(node.from, to)), block: true }).range(
              node.from,
              to,
            ),
          );
          return false;
        }
        case "Image": {
          if (!isBlockImage(state, node)) return false;
          const parts = imageParts(state, node.node);
          if (!parts) return false;
          const ln = state.doc.lineAt(node.from);
          const widget = new ImageWidget(host.imageSrc(parts.src), parts.alt, true, node.from - ln.from + 2);
          // While editing its source, the picture stays in view below it.
          out.push(
            touches(state, node.from, node.to)
              ? Decoration.widget({ widget, block: true, side: 1 }).range(ln.to)
              : Decoration.replace({ widget, block: true }).range(ln.from, ln.to),
          );
          return false;
        }
        case "FencedCode": {
          if (!ownsLines(state, node)) return false;
          const code = mermaidCode(state, node.node);
          if (code == null) return false;
          const first = state.doc.lineAt(node.from);
          const to = state.doc.lineAt(node.to).to;
          // A press puts the caret at the start of the first line inside.
          const caret = Math.min(first.to + 1, to) - node.from;
          // While editing its source, the diagram stays in view below it.
          out.push(
            touchesLines(state, node.from, node.to)
              ? Decoration.widget({ widget: new MermaidWidget(code, true, caret), block: true, side: 1 }).range(to)
              : Decoration.replace({ widget: new MermaidWidget(code, false, caret), block: true }).range(node.from, to),
          );
          return false;
        }
        // Nothing block-level lives inside these.
        case "Paragraph":
          return node.node.getChild("Image") != null;
        case "CodeBlock":
        case "ATXHeading1":
        case "ATXHeading2":
        case "ATXHeading3":
        case "ATXHeading4":
        case "ATXHeading5":
        case "ATXHeading6":
          return false;
      }
      return undefined;
    },
  });
  return Decoration.set(out, true);
}

const blockField = StateField.define<DecorationSet>({
  create: buildBlocks,
  update(deco, tr) {
    const stale =
      tr.docChanged ||
      tr.selection ||
      tr.effects.some((e) => e.is(setFocused)) ||
      syntaxTree(tr.state) !== syntaxTree(tr.startState) ||
      tr.state.facet(noteHost) !== tr.startState.facet(noteHost);
    return stale ? buildBlocks(tr.state) : deco;
  },
  provide: (f) => EditorView.decorations.from(f),
});

// ── Tables: atomic, entered by key ─────────────────────────────────────────

function tablesIn(blocks: DecorationSet): DecorationSet {
  const out: Range<Decoration>[] = [];
  for (const iter = blocks.iter(); iter.value; iter.next()) {
    if (iter.value.spec.widget instanceof TableWidget) out.push(iter.value.range(iter.from, iter.to));
  }
  return Decoration.set(out);
}

/** The drawn tables alone. Atomic, so the caret and selection skip their
 *  source; it rests only at a table's start or end. */
const tableField = StateField.define<DecorationSet>({
  create: (state) => tablesIn(state.field(blockField)),
  update(tables, tr) {
    const blocks = tr.state.field(blockField);
    return blocks === tr.startState.field(blockField, false) ? tables : tablesIn(blocks);
  },
  provide: (f) => EditorView.atomicRanges.of((view) => view.state.field(f)),
});

interface Span {
  from: number;
  to: number;
}

/** The drawn table starting (`from`) or ending (`to`) exactly at `pos`. */
function tableAt(state: EditorState, pos: number, edge: "from" | "to"): Span | null {
  let found = null as Span | null;
  state.field(tableField, false)?.between(pos, pos, (from, to) => {
    if ((edge === "from" ? from : to) !== pos) return;
    found = { from, to };
    return false;
  });
  return found;
}

/** The drawn table on the line just above the one holding `pos` (`up`), or
 *  just below it. */
function tableBeside(state: EditorState, pos: number, up: boolean): Span | null {
  const ln = state.doc.lineAt(pos);
  if (up) return ln.from > 0 ? tableAt(state, ln.from - 1, "to") : null;
  return ln.to < state.doc.length ? tableAt(state, ln.to + 1, "from") : null;
}

/** The lone empty caret, or null. */
function caretOf(state: EditorState): number | null {
  const { ranges, main } = state.selection;
  return ranges.length === 1 && main.empty ? main.head : null;
}

/** An arrow into a table: from the line under it (its first visual line for
 *  ↑, its start for ←) or the table's end into the last row; from the line
 *  over it or the table's start into the header. */
function intoTable(dir: "up" | "down" | "left" | "right"): Command {
  const back = dir === "up" || dir === "left";
  return (view) => {
    const { state } = view;
    const head = caretOf(state);
    if (head == null) return false;
    let table = tableAt(state, head, back ? "to" : "from");
    let x: number | undefined;
    if (!table) {
      const ln = state.doc.lineAt(head);
      table = tableBeside(state, head, back);
      if (!table) return false;
      if (dir === "left" || dir === "right") {
        if (head !== (back ? ln.from : ln.to)) return false;
      } else {
        const next = view.moveVertically(EditorSelection.cursor(head), !back).head;
        if (back ? next >= ln.from : next <= ln.to) return false;
        x = view.coordsAtPos(head)?.left;
      }
    }
    const col = x != null ? { x } : dir === "left" ? "last" : "first";
    return enterTable(view, table.from, back ? "last" : "first", col, dir === "right" ? "start" : "end");
  };
}

/** Backspace from the line under a table (or its end), Delete from the line
 *  over it (or its start): into the table, never joining a row or deleting the
 *  atomic whole. An empty line between goes too, unless text lies beyond it. */
function deleteIntoTable(forward: boolean): Command {
  return (view) => {
    const { state } = view;
    const { doc } = state;
    const head = caretOf(state);
    if (head == null) return false;
    let table = tableAt(state, head, forward ? "from" : "to");
    let change: ChangeSpec | null = null;
    if (!table) {
      const ln = doc.lineAt(head);
      if (head !== (forward ? ln.to : ln.from)) return false;
      table = tableBeside(state, head, !forward);
      if (!table) return false;
      const beyond = forward ? (ln.number > 1 ? doc.line(ln.number - 1) : null) : ln.number < doc.lines ? doc.line(ln.number + 1) : null;
      if (!ln.length && !beyond?.text.trim()) {
        change = forward ? { from: ln.from, to: ln.to + 1 } : { from: ln.from - 1, to: ln.to };
      }
    }
    if (change) view.dispatch({ changes: change, scrollIntoView: true, userEvent: "delete" });
    // Deleting the line over a table moves it up by that newline.
    const base = change && forward ? table.from - 1 : table.from;
    const entered = enterTable(view, base, forward ? "first" : "last", forward ? "first" : "last", forward ? "start" : "end");
    return entered || change != null;
  };
}

const tableKeys = Prec.highest(
  keymap.of([
    { key: "ArrowUp", run: intoTable("up") },
    { key: "ArrowDown", run: intoTable("down") },
    { key: "ArrowLeft", run: intoTable("left") },
    { key: "ArrowRight", run: intoTable("right") },
    { key: "Backspace", run: deleteIntoTable(false) },
    { key: "Delete", run: deleteIntoTable(true) },
  ]),
);

/** How text put at `pos` is padded to stay off a drawn table: GFM reads a line
 *  straight under a table as a row, so text at its end or on the empty line
 *  under it starts after a blank line; at its start, it gets its own line. */
function tablePadding(state: EditorState, pos: number): [string, string] | null {
  if (tableAt(state, pos, "to")) return ["\n\n", ""];
  if (tableAt(state, pos, "from")) return ["", "\n"];
  if (!state.doc.lineAt(pos).length && tableBeside(state, pos, true)) return ["\n", ""];
  return null;
}

/** Typing beside a table; a cell's own writes are dispatched, not typed. */
const tableTyping = EditorView.inputHandler.of((view, from, to, text) => {
  if (from !== to || view.composing || caretOf(view.state) !== from) return false;
  const pad = tablePadding(view.state, from);
  if (!pad) return false;
  view.dispatch({
    changes: { from, insert: pad[0] + text + pad[1] },
    selection: { anchor: from + pad[0].length + text.length },
    scrollIntoView: true,
    userEvent: "input.type",
  });
  return true;
});

/** Pasting beside a table, which skips the input handler. */
const tablePaste = EditorState.transactionFilter.of((tr) => {
  if (!tr.docChanged || !tr.isUserEvent("input.paste")) return tr;
  const pos = caretOf(tr.startState);
  if (pos == null) return tr;
  const parts: { from: number; to: number; text: string }[] = [];
  tr.changes.iterChanges((from, to, _a, _b, inserted) => parts.push({ from, to, text: inserted.toString() }));
  if (parts.length !== 1 || parts[0].from !== pos || parts[0].to !== pos) return tr;
  const pad = tablePadding(tr.startState, pos);
  if (!pad) return tr;
  const { text } = parts[0];
  return {
    changes: { from: pos, insert: pad[0] + text + pad[1] },
    selection: { anchor: pos + pad[0].length + text.length },
    scrollIntoView: true,
    userEvent: "input.paste",
  };
});

// ── Inline decorations (view plugin) ───────────────────────────────────────

const HEADING = /^ATXHeading(\d)$/;
const SETEXT = /^SetextHeading(\d)$/;

const INLINE_STYLE: Record<string, { cls: string; mark: string }> = {
  Emphasis: { cls: "cm-em", mark: "EmphasisMark" },
  StrongEmphasis: { cls: "cm-strong", mark: "EmphasisMark" },
  Strikethrough: { cls: "cm-strike", mark: "StrikethroughMark" },
  InlineCode: { cls: "cm-inline-code", mark: "CodeMark" },
};

/** Parents that own their `URL`; any other `URL` is a bare GFM autolink. */
const LINKISH = new Set(["Link", "Image", "Autolink", "LinkReference"]);

function buildInline(view: EditorView): DecorationSet {
  const { state } = view;
  const host = state.facet(noteHost);
  const out: Range<Decoration>[] = [];
  const { from, to } = view.viewport;

  /** Hide `[from, to)` plus one following space, as after `#` or `>`. */
  const hideWithSpace = (a: number, b: number) => {
    const end = state.sliceDoc(b, b + 1) === " " ? b + 1 : b;
    if (end > a) out.push(hide.range(a, end));
  };

  /** Line decorations for every line in `a..b`, first and last flagged. */
  const eachLine = (a: number, b: number, cls: string, ends = false) => {
    const first = state.doc.lineAt(a).number;
    const last = state.doc.lineAt(b).number;
    for (let n = first; n <= last; n++) {
      const ln = state.doc.line(n);
      if (ln.to < from || ln.from > to) continue;
      let c = cls;
      if (ends && n === first) c += ` ${cls}-first`;
      if (ends && n === last) c += ` ${cls}-last`;
      out.push(line(c).range(ln.from));
    }
  };

  syntaxTree(state).iterate({
    from,
    to,
    enter: (node) => {
      const name = node.name;

      const heading = HEADING.exec(name) ?? SETEXT.exec(name);
      if (heading) {
        out.push(line(`cm-h${heading[1]}`).range(state.doc.lineAt(node.from).from));
        if (HEADING.test(name) && !touchesLines(state, node.from, node.to)) {
          for (const m of node.node.getChildren("HeaderMark")) {
            // A closing run (`## Title ##`) takes the space before it instead.
            if (m.from === node.from) hideWithSpace(m.from, m.to);
            else {
              const before = state.sliceDoc(m.from - 1, m.from) === " " ? m.from - 1 : m.from;
              out.push(hide.range(before, m.to));
            }
          }
        }
        return undefined;
      }

      const inline = INLINE_STYLE[name];
      if (inline) {
        out.push(mark(inline.cls).range(node.from, node.to));
        if (!touches(state, node.from, node.to)) {
          for (const m of node.node.getChildren(inline.mark)) out.push(hide.range(m.from, m.to));
        }
        return name === "InlineCode" ? false : undefined;
      }

      switch (name) {
        case "Link": {
          const url = node.node.getChild("URL");
          const marks = node.node.getChildren("LinkMark");
          if (!url || marks.length < 2 || marks[0].to >= marks[1].from) return undefined;
          out.push(mark("cm-link").range(marks[0].to, marks[1].from));
          if (!touches(state, node.from, node.to)) {
            out.push(hide.range(node.from, marks[0].to));
            out.push(hide.range(marks[1].from, node.to));
          }
          return undefined;
        }
        case "Autolink": {
          const url = node.node.getChild("URL");
          if (!url) return false;
          out.push(mark("cm-link").range(url.from, url.to));
          if (!touches(state, node.from, node.to)) {
            out.push(hide.range(node.from, url.from));
            out.push(hide.range(url.to, node.to));
          }
          return false;
        }
        case "URL": {
          if (!LINKISH.has(node.node.parent?.name ?? "")) {
            out.push(mark("cm-link").range(node.from, node.to));
          }
          return false;
        }
        case "Image": {
          if (isBlockImage(state, node) || touches(state, node.from, node.to)) return false;
          const parts = imageParts(state, node.node);
          if (!parts) return false;
          out.push(
            Decoration.replace({
              widget: new ImageWidget(host.imageSrc(parts.src), parts.alt, false, 2),
            }).range(node.from, node.to),
          );
          return false;
        }
        case "ListMark": {
          const item = node.node.parent;
          const list = item?.parent;
          if (list?.name === "OrderedList") {
            out.push(mark("cm-list-number").range(node.from, node.to));
            return false;
          }
          if (list?.name !== "BulletList" || touchesLines(state, node.from, node.to)) return false;
          // A task's checkbox stands in for its bullet.
          if (item?.getChild("Task")) hideWithSpace(node.from, node.to);
          else out.push(Decoration.replace({ widget: new BulletWidget() }).range(node.from, node.to));
          return false;
        }
        case "Task": {
          const marker = node.node.getChild("TaskMarker");
          if (!marker) return undefined;
          const checked = /x/i.test(state.sliceDoc(marker.from, marker.to));
          if (!touches(state, marker.from, marker.to)) {
            out.push(Decoration.replace({ widget: new CheckboxWidget(checked) }).range(marker.from, marker.to));
          }
          if (checked && marker.to < node.to) out.push(mark("cm-task-done").range(marker.to, node.to));
          return undefined;
        }
        case "Blockquote": {
          eachLine(node.from, node.to, "cm-quote");
          return undefined;
        }
        case "QuoteMark": {
          if (!touchesLines(state, node.from, node.to)) hideWithSpace(node.from, node.to);
          return false;
        }
        case "FencedCode": {
          if (isDrawnMermaid(state, node) && !touchesLines(state, node.from, node.to)) return false;
          eachLine(node.from, node.to, "cm-codeblock", true);
          const marks = node.node.getChildren("CodeMark");
          if (!marks.length || touchesLines(state, node.from, node.to)) {
            const info = node.node.getChild("CodeInfo");
            if (info) out.push(mark("cm-code-info").range(info.from, info.to));
            return false;
          }
          // The header takes the opening fence's place on its own line, so
          // revealing the fence moves nothing under the pointer.
          const read = (a: number, b: number) => state.sliceDoc(a, b);
          const lang = fenceLanguage(node.node, read);
          const head = state.doc.lineAt(marks[0].from);
          const header = new CodeHeaderWidget(lang?.label ?? "Plain text", lang?.auto ?? false, fenceCode(node.node, read));
          out.push(Decoration.replace({ widget: header }).range(marks[0].from, head.to));
          const close = marks.length > 1 ? marks[marks.length - 1] : null;
          if (close && close.from > head.to) {
            out.push(hide.range(close.from, close.to));
            out.push(line("cm-codeblock-close-hidden").range(state.doc.lineAt(close.from).from));
          }
          return false;
        }
        case "CodeBlock": {
          eachLine(node.from, node.to, "cm-codeblock", true);
          return false;
        }
        case "InlineMath": {
          if (touches(state, node.from, node.to)) return false;
          const parts = mathParts(state, node.node);
          if (parts) {
            out.push(
              Decoration.replace({ widget: new MathWidget(parts.source, false, parts.caret) }).range(
                node.from,
                node.to,
              ),
            );
          }
          return false;
        }
        case "BlockMath": {
          // Inside a quote or list item the field cannot draw it; one line
          // still renders inline, several stay source.
          if (ownsLines(state, node) || touches(state, node.from, node.to)) return false;
          if (state.doc.lineAt(node.from).number !== state.doc.lineAt(node.to).number) return false;
          const parts = mathParts(state, node.node);
          if (parts) {
            out.push(
              Decoration.replace({ widget: new MathWidget(parts.source, true, parts.caret) }).range(
                node.from,
                node.to,
              ),
            );
          }
          return false;
        }
      }
      return undefined;
    },
  });
  return Decoration.set(out, true);
}

const inlinePlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildInline(view);
    }
    update(u: ViewUpdate) {
      if (
        u.docChanged ||
        u.selectionSet ||
        u.viewportChanged ||
        u.focusChanged ||
        u.transactions.some((tr) => tr.effects.some((e) => e.is(setFocused))) ||
        syntaxTree(u.state) !== syntaxTree(u.startState) ||
        u.state.facet(noteHost) !== u.startState.facet(noteHost)
      ) {
        this.decorations = buildInline(u.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

/** Tell a freshly configured Live mode whether the editor has focus: the
 *  field starts unfocused, and only a focus change would correct it. */
export function syncLiveFocus(view: EditorView): void {
  if (view.state.field(focusedField, false) !== undefined) {
    view.dispatch({ effects: setFocused.of(view.hasFocus) });
  }
}

/** Everything Live mode adds over Raw; swapped through a compartment. */
export function livePreview(): Extension {
  return [focusedField, trackFocus, blockField, tableField, tableKeys, tableTyping, tablePaste, inlinePlugin];
}
