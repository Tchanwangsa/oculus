import type { EditorView } from "@codemirror/view";
import katex from "katex";

import "katex/dist/katex.min.css";

import { mathFieldFocused } from "../../core/liveFocus";
import { MATH_ARRAYSTRETCH, mathLiveReady, noteMathPress, staticMath } from "../../math/field/mathField";
import { ancestorAt } from "../../syntax/syntax";
import { SourceWidget } from "./source";

/** Rendered KaTeX by display flag and source; a note re-renders often. */
const katexCache = new Map<string, string | null>();
const KATEX_CACHE_MAX = 500;

/** `\left[ \begin{array}…\end{array} \right]` drawn like `bmatrix`: an
 *  array keeps `\arraycolsep` outside its first and last columns (matrices
 *  drop it), which reads as a gap inside the brackets. Render-time only, and
 *  the field draws it the same (`patchArrays` in `math/field/mathField`). */
const LEFT_BEFORE = /\\left\s*(?:\\[a-zA-Z]+|\\.|[^\s\\])\s*$/;
const BEGIN = "\\begin{array}";
const END = "\\end{array}";

function hugArrays(source: string): string {
  if (!source.includes(BEGIN)) return source;
  let out = "";
  let done = 0;
  for (let at = source.indexOf(BEGIN); at >= 0; at = source.indexOf(BEGIN, at + 1)) {
    if (at < done || !LEFT_BEFORE.test(source.slice(0, at))) continue;
    // The matching `\end{array}`, past any nested array.
    let depth = 0;
    let end = -1;
    for (let i = at; i < source.length; i++) {
      if (source.startsWith(BEGIN, i)) depth++;
      else if (source.startsWith(END, i) && --depth === 0) {
        end = i + END.length;
        break;
      }
    }
    if (end < 0 || !/^\s*\\right/.test(source.slice(end))) continue;
    out += `${source.slice(done, at)}\\kern-0.5em${source.slice(at, end)}\\kern-0.5em`;
    done = end;
  }
  return out + source.slice(done);
}

/** KaTeX HTML for `source`, or null if it does not parse. Matrix rows get
 *  `MATH_ARRAYSTRETCH`, as the visual field draws them (`math/field/mathField`). */
function renderMath(source: string, display: boolean): string | null {
  const key = `${display ? "D" : "I"}${source}`;
  const hit = katexCache.get(key);
  if (hit !== undefined) return hit;
  let html: string | null;
  try {
    // A fresh macro table each time: KaTeX writes the source's `\def`s into it.
    const macros = { "\\arraystretch": String(MATH_ARRAYSTRETCH) };
    html = katex.renderToString(hugArrays(source), { displayMode: display, throwOnError: true, macros });
  } catch {
    html = null;
  }
  if (katexCache.size >= KATEX_CACHE_MAX) katexCache.clear();
  katexCache.set(key, html);
  return html;
}

/** The maths node a rendering starts at, and where its widget ends (a
 *  block's closing line end). */
function mathSpan(view: EditorView, start: number, block: boolean): { from: number; to: number } | null {
  const node = ancestorAt(view.state, start, (n) => n.name === "InlineMath" || n.name === "BlockMath", [1]);
  if (!node || node.from !== start) return null;
  return { from: node.from, to: block ? view.state.doc.lineAt(node.to).to : node.to };
}

/** The widget each rendering was drawn or last updated from. */
const drawnMath = new WeakMap<HTMLElement, MathWidget>();

/**
 * Rendered maths, one unit to the selection: its range is atomic in Live mode
 * (`live-preview/livePreview`), so the selection layer highlights inline maths like a
 * word. A block (`block`, drawn by the block layer) is marked `selected`
 * while a selection covers it and fills whole. It has a caret spot before
 * and after it: a press in its padding above or below the formula rests the
 * caret there, drawn beside the formula's first or last row (`coordsAt`).
 * Any other press opens the visual field where it landed; Shift with a press
 * extends the selection over the maths.
 */
export class MathWidget extends SourceWidget {
  /** Drawn by MathLive (`staticMath`) once it has loaded, else by KaTeX; a
   *  rebuild after it loads draws again. */
  readonly mathLive = mathLiveReady();

  constructor(
    readonly source: string,
    readonly display: boolean,
    caret: number,
    readonly block = false,
    readonly selected = false,
  ) {
    super(caret);
  }

  eq(other: MathWidget) {
    return this.sameMath(other) && other.selected === this.selected;
  }

  sameMath(other: MathWidget) {
    return (
      other.source === this.source &&
      other.display === this.display &&
      other.caret === this.caret &&
      other.block === this.block &&
      other.mathLive === this.mathLive
    );
  }

  toDOM(view: EditorView) {
    const dom = document.createElement(this.display ? "div" : "span");
    dom.className = this.display ? "cm-math cm-math-display" : "cm-math";
    dom.classList.toggle("cm-math-selected", this.selected);
    drawnMath.set(dom, this);
    const ml = this.mathLive ? staticMath(this.source, this.display) : null;
    const html = ml || !this.source.trim() ? null : renderMath(this.source, this.display);
    if (ml) {
      dom.append(ml);
    } else if (html) {
      dom.innerHTML = html;
    } else {
      // Bad or empty LaTeX shows its source, so the mistake is findable.
      dom.classList.add("cm-math-error");
      dom.textContent = this.source.trim() ? this.source : "Empty equation";
    }
    // Before `reveal`'s handler, which it pre-empts for Shift and a block's
    // edges; otherwise the field that replaces this rendering puts its caret
    // where the press landed.
    dom.addEventListener("mousedown", (ev) => {
      const e = ev as MouseEvent;
      if (e.button !== 0) return;
      // Out of an open maths field before the press removes it: focused after,
      // the note has WebKit scroll to the removed field's DOM selection.
      if (mathFieldFocused()) view.focus();
      if (this.pressAround(e, dom, view)) {
        e.preventDefault();
        e.stopImmediatePropagation();
        view.focus();
        return;
      }
      noteMathPress(e.clientX, e.clientY, dom);
    });
    this.reveal(dom, view);
    return dom;
  }

  /** Only the highlight changed: keep the rendering. */
  updateDOM(dom: HTMLElement) {
    const old = drawnMath.get(dom);
    if (!old || !old.sameMath(this)) return false;
    dom.classList.toggle("cm-math-selected", this.selected);
    drawnMath.set(dom, this);
    return true;
  }

  /** Shift-press: the selection runs on over the maths. A block's padding
   *  above or below the formula: the caret before or after it. Returns
   *  whether it moved the selection. */
  private pressAround(e: MouseEvent, dom: HTMLElement, view: EditorView): boolean {
    const span = mathSpan(view, view.posAtDOM(dom), this.block);
    if (!span) return false;
    if (e.shiftKey) {
      let { anchor } = view.state.selection.main;
      if (anchor > span.from && anchor < span.to) anchor = span.from;
      view.dispatch({ selection: { anchor, head: anchor >= span.to ? span.from : span.to } });
      return true;
    }
    if (!this.block) return false;
    // The box a field opened here would take, or KaTeX's.
    const ink = dom.querySelector(".cm-math-ml, .katex-display")?.getBoundingClientRect();
    if (!ink || (e.clientY >= ink.top && e.clientY <= ink.bottom)) return false;
    view.dispatch({ selection: { anchor: e.clientY < ink.top ? span.from : span.to } });
    return true;
  }

  /** A caret at a block's start or end, one text line tall beside the first
   *  or last row, not the block's full height at its edge. */
  coordsAt(dom: HTMLElement, pos: number) {
    if (!this.block) return null;
    const rows = mathRows(dom);
    const row = pos === 0 ? rows[0] : rows[rows.length - 1];
    if (!row) return null;
    const height = parseFloat(getComputedStyle(dom).lineHeight) || row.bottom - row.top;
    const x = pos === 0 ? row.left : row.right;
    const top = (row.top + row.bottom) / 2 - height / 2;
    return { left: x, right: x, top, bottom: top + height };
  }

  ignoreEvent(e: Event) {
    // The note's copy and paste, which write and read its source.
    return !(e.type === "copy" || e.type === "cut" || e.type === "paste");
  }

  get estimatedHeight() {
    return this.display ? 56 : -1;
  }
}

/** A `\displaylines` table's rows in MathLive's static markup: each row's
 *  cell, stacked in the column's first vlist row. */
const ML_LINES =
  ".ML__latex > .ML__base > .ML__multiline_environment > .col-align-l:only-child > .ML__vlist-t > " +
  ".ML__vlist-r:first-child > .ML__vlist > span > :not(.ML__pstrut)";

type Row = { left: number; right: number; top: number; bottom: number };

/** The boxes of a display rendering's rows (its top-level `\\` lines). */
function mathRows(dom: HTMLElement): Row[] {
  const rect = (el: Element): Row => {
    const r = el.getBoundingClientRect();
    return { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
  };
  const ml = dom.querySelector(".ML__latex");
  if (ml) {
    const lines = dom.querySelectorAll(ML_LINES);
    return lines.length ? [...lines].map(rect) : [rect(ml)];
  }
  // KaTeX: the runs between `.newline`s.
  const rows: Row[] = [];
  let row: Row | null = null;
  for (const el of dom.querySelectorAll(".katex-display .katex-html > *")) {
    if (el.classList.contains("newline")) {
      row = null;
      continue;
    }
    const r = el.getBoundingClientRect();
    if (!r.width) continue;
    if (!row) rows.push((row = { left: r.left, right: r.right, top: r.top, bottom: r.bottom }));
    else {
      row.left = Math.min(row.left, r.left);
      row.right = Math.max(row.right, r.right);
      row.top = Math.min(row.top, r.top);
      row.bottom = Math.max(row.bottom, r.bottom);
    }
  }
  return rows;
}
