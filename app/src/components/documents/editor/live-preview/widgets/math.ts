import { StateEffect } from "@codemirror/state";
import { ViewPlugin, type EditorView } from "@codemirror/view";

import "@/styles/katex/katex.min.css";

import { MathField, mathsReady, onMathsReady } from "@/lib/maths";
import { formulaOf } from "@/lib/markdown/mathSelection/formula";
import { WholeBands } from "@/lib/markdown/mathSelection/whole";
import { mathFieldFocused } from "../../core/liveFocus";
import { noteMathPress } from "../../math/field/mathField";
import { fieldHtml } from "../../math/field/mathView/render";
import { ancestorAt } from "../../syntax/syntax";
import { SourceWidget } from "./source";

/** Rendered KaTeX by display flag and source; a note re-renders often. */
const katexCache = new Map<string, string | null>();
const KATEX_CACHE_MAX = 500;

/** KaTeX HTML for `source` as the visual field draws it (`fieldHtml`: the
 *  source map on, matrix rows `MATH_ARRAYSTRETCH`), or null if it does not
 *  render. Only once `mathsReady()`. */
function renderMath(source: string, display: boolean): string | null {
  const key = `${display ? "D" : "I"}${source}`;
  const hit = katexCache.get(key);
  if (hit !== undefined) return hit;
  let html: string | null;
  try {
    html = fieldHtml(source, display);
  } catch {
    html = null;
  }
  if (katexCache.size >= KATEX_CACHE_MAX) katexCache.clear();
  katexCache.set(key, html);
  return html;
}

/** The maths engine came up or went down (`lib/maths`): renderings redraw. */
export const mathsSettled = StateEffect.define<null>();

/** Tells each Live editor when the engine's readiness changes, so maths drawn
 *  as a placeholder before it loaded is drawn again. */
export const mathsWatcher = ViewPlugin.fromClass(
  class {
    readonly off: () => void;
    constructor(view: EditorView) {
      this.off = onMathsReady(() => view.dispatch({ effects: mathsSettled.of(null) }));
    }
    destroy() {
      this.off();
    }
  },
);

/** The maths node a rendering starts at, and where its widget ends (a
 *  block's closing line end). */
function mathSpan(view: EditorView, start: number, block: boolean): { from: number; to: number } | null {
  const node = ancestorAt(view.state, start, (n) => n.name === "InlineMath" || n.name === "BlockMath", [1]);
  if (!node || node.from !== start) return null;
  return { from: node.from, to: block ? view.state.doc.lineAt(node.to).to : node.to };
}

/** The widget each rendering was drawn or last updated from. */
const drawnMath = new WeakMap<HTMLElement, MathWidget>();

/** The bands over each rendering a selection covers. */
const covered = new WeakMap<HTMLElement, WholeBands>();

/** Bands over the whole rendering while a selection covers it, as over a
 *  chat formula a text selection takes (`WholeBands`); the note's own
 *  highlight skips it (`selectionGaps`). */
function showCovered(dom: HTMLElement, w: MathWidget, on: boolean) {
  const bands = covered.get(dom);
  if (!on || !w.rendered()) {
    bands?.close();
    covered.delete(dom);
    return;
  }
  if (bands) return;
  const formula = formulaOf(dom.querySelector(".katex-display") ?? dom.querySelector(".katex") ?? dom);
  if (!formula) return;
  let slots: MathField["slots"] = [];
  try {
    const field = MathField.open(w.source, w.display);
    slots = field.slots;
    field.free();
  } catch {
    // The rendering's atoms still get bands, a line each.
  }
  covered.set(dom, new WholeBands(formula, slots));
}

/**
 * Rendered maths, one unit to the selection: its range is atomic in Live mode
 * (`live-preview/livePreview`), and while a selection covers it (`selected`)
 * it draws bands over its atoms, which the note's highlight leaves out. A
 * block (`block`) is drawn by the block layer. It has a caret spot before
 * and after it: a press in its padding above or below the formula rests the
 * caret there, drawn beside the formula's first or last row (`coordsAt`).
 * Any other press opens the visual field where it landed; Shift with a press
 * extends the selection over the maths.
 */
export class MathWidget extends SourceWidget {
  /** Drawn as a placeholder until the maths engine is ready; a rebuild after
   *  it is (`mathsSettled`) draws again. */
  readonly maths = mathsReady();

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
      other.maths === this.maths
    );
  }

  toDOM(view: EditorView) {
    const dom = document.createElement(this.display ? "div" : "span");
    dom.className = this.display ? "cm-math cm-math-display" : "cm-math";
    drawnMath.set(dom, this);
    const pending = !this.maths && !!this.source.trim();
    const html = pending || !this.source.trim() ? null : renderMath(this.source, this.display);
    if (pending) {
      dom.classList.add("cm-math-pending");
      dom.textContent = this.source;
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
    showCovered(dom, this, this.selected);
    return dom;
  }

  /** Drawn as maths, not as its source (pending, or LaTeX that doesn't
   *  render). */
  rendered(): boolean {
    return this.maths && !!this.source.trim() && renderMath(this.source, this.display) != null;
  }

  destroy(dom: HTMLElement) {
    showCovered(dom, this, false);
  }

  /** Only the highlight changed: keep the rendering. */
  updateDOM(dom: HTMLElement) {
    const old = drawnMath.get(dom);
    if (!old || !old.sameMath(this)) return false;
    showCovered(dom, this, this.selected);
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
    const ink = dom.querySelector(".katex-display")?.getBoundingClientRect();
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

type Row = { left: number; right: number; top: number; bottom: number };

/** The boxes of a display rendering's rows (its top-level `\\` lines). */
function mathRows(dom: HTMLElement): Row[] {
  // The runs between `.katex-newline`s.
  const rows: Row[] = [];
  let row: Row | null = null;
  for (const el of dom.querySelectorAll(".katex-display .katex-html > *")) {
    if (el.classList.contains("katex-newline")) {
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
