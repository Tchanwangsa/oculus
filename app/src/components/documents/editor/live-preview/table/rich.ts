import type { SyntaxNode } from "@lezer/common";

import { mathsReady, onMathsReady } from "@/lib/maths";
import { noteLanguage } from "../../core/language";
import { inlineCodeCitation } from "../../syntax/mentionSyntax";
import { CitationWidget } from "../widgets/citation";
import { renderMath } from "../widgets/math";
import { cellValue } from "./state";

/**
 * A cell that holds markup draws it, as the rest of the note does: marks,
 * code, links, file chips and maths. The cell's own element stays the editor
 * (the source, laid over the drawing and invisible); focusing it swaps the
 * drawing for the source, and leaving it draws again. A cell without markup
 * has no drawing at all.
 */

const MARKUP = /[`*_~$\\[]/;
const MATH = new Set(["InlineMath", "ParenMath"]);

interface Built {
  nodes: Node[];
  /** Something was drawn that plain text would not show. */
  drawn: boolean;
  /** Maths wanted drawing before the engine was ready. */
  waiting: boolean;
  cleanups: (() => void)[];
}

const disposers = new WeakMap<HTMLElement, (() => void)[]>();

function content(node: SyntaxNode, from: number, to: number, src: string, b: Built, skip: ReadonlySet<string>): Node[] {
  const out: Node[] = [];
  let at = from;
  for (let ch = node.firstChild; ch; ch = ch.nextSibling) {
    if (ch.to <= from || ch.from >= to) continue;
    if (ch.from > at) out.push(document.createTextNode(src.slice(at, ch.from)));
    at = ch.to;
    if (!skip.has(ch.name)) out.push(...one(ch, src, b));
  }
  if (at < to) out.push(document.createTextNode(src.slice(at, to)));
  return out;
}

function wrap(tag: string, children: Node[], b: Built, cls?: string): Node[] {
  const el = document.createElement(tag);
  if (cls) el.className = cls;
  el.append(...children);
  b.drawn = true;
  return [el];
}

const MARKS = new Set(["EmphasisMark", "StrikethroughMark"]);

function one(node: SyntaxNode, src: string, b: Built): Node[] {
  switch (node.name) {
    case "StrongEmphasis":
      return wrap("strong", content(node, node.from, node.to, src, b, MARKS), b);
    case "Emphasis":
      return wrap("em", content(node, node.from, node.to, src, b, MARKS), b);
    case "Strikethrough":
      return wrap("s", content(node, node.from, node.to, src, b, MARKS), b);
    case "InlineCode": {
      const cite = inlineCodeCitation(src.slice(node.from, node.to));
      if (cite) {
        const widget = new CitationWidget(cite);
        const dom = widget.toDOM();
        b.cleanups.push(() => widget.destroy(dom));
        b.drawn = true;
        return [dom];
      }
      const marks = node.getChildren("CodeMark");
      if (marks.length < 2) break;
      const text = src.slice(marks[0].to, marks[marks.length - 1].from).replace(/^ (.*[^ ].*) $/, "$1");
      return wrap("span", [document.createTextNode(text)], b, "cm-inline-code");
    }
    case "Link": {
      const marks = node.getChildren("LinkMark");
      if (marks.length < 2) break;
      return wrap("span", content(node, marks[0].to, marks[1].from, src, b, MARKS), b, "cm-table-link");
    }
    case "Escape":
      b.drawn = true;
      return [document.createTextNode(src.slice(node.from + 1, node.to))];
    default:
      if (MATH.has(node.name)) return maths(node, src, b);
      if (node.firstChild) return content(node, node.from, node.to, src, b, new Set());
  }
  return [document.createTextNode(src.slice(node.from, node.to))];
}

function maths(node: SyntaxNode, src: string, b: Built): Node[] {
  const marks = node.getChildren("MathMark");
  const raw = document.createTextNode(src.slice(node.from, node.to));
  if (marks.length < 2) return [raw];
  const tex = src.slice(marks[0].to, marks[marks.length - 1].from).trim();
  if (!tex) return [raw];
  if (!mathsReady()) {
    b.waiting = true;
    return [raw];
  }
  const html = renderMath(tex, false);
  if (html == null) return [raw];
  const span = document.createElement("span");
  span.className = "cm-table-math";
  span.innerHTML = html;
  b.drawn = true;
  return [span];
}

function build(text: string): Built | null {
  if (!MARKUP.test(text)) return null;
  const tree = noteLanguage.parser.parse(text);
  const b: Built = { nodes: [], drawn: false, waiting: false, cleanups: [] };
  b.nodes = content(tree.topNode, 0, text.length, text, b, new Set());
  if (b.drawn || b.waiting) return b;
  return null;
}

function richOf(td: HTMLElement): HTMLElement | null {
  return td.querySelector<HTMLElement>(":scope > .cm-table-rich");
}

function dispose(rich: HTMLElement) {
  for (const fn of disposers.get(rich) ?? []) fn();
  disposers.delete(rich);
}

/** Draw `cell`'s markup (`text`, its source) in its place, or show the
 *  source: a focused cell is always source, and one with no markup too.
 *  A `focusout` handler passes `editing` false: WebKit still reports the
 *  cell as active while it is blurring. */
export function paintRich(cell: HTMLElement, text: string, editing = cell === cell.ownerDocument.activeElement) {
  const td = cell.parentElement;
  if (!td) return;
  let rich = richOf(td);
  const ready = String(mathsReady());
  const focused = editing;
  if (!focused && rich && rich.dataset.src === text && rich.dataset.ready === ready) {
    td.classList.add("cm-cell-rich");
    return;
  }
  const built = focused ? null : build(text);
  if (!built) {
    if (rich) dispose(rich);
    rich?.remove();
    td.classList.remove("cm-cell-rich");
    return;
  }
  if (!rich) {
    rich = document.createElement("div");
    rich.className = "cm-table-rich";
    rich.setAttribute("aria-hidden", "true");
    td.appendChild(rich);
  }
  dispose(rich);
  rich.replaceChildren(...built.nodes);
  rich.dataset.src = text;
  rich.dataset.ready = ready;
  disposers.set(rich, built.cleanups);
  td.classList.add("cm-cell-rich");
  if (built.waiting) {
    const off = onMathsReady(() => {
      off();
      if (cell.isConnected) paintRich(cell, cellValue(cell));
    });
  }
}

/** A cell takes focus: its source replaces the drawing. */
export function showSource(cell: HTMLElement) {
  cell.parentElement?.classList.remove("cm-cell-rich");
}

/** Release every drawing under `root` (a grid about to be replaced, or a
 *  table leaving the page), unmounting its chips. */
export function disposeRich(root: HTMLElement) {
  for (const rich of root.querySelectorAll<HTMLElement>(".cm-table-rich")) dispose(rich);
}
