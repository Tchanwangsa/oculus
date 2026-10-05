import { WidgetType, type EditorView } from "@codemirror/view";
import katex from "katex";
import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";

import "katex/dist/katex.min.css";

import { CitationCode } from "@/components/markdown/Citation";
import { diagramBounds, mermaidId, renderMermaid, type Diagram } from "@/components/markdown/mermaidRender";
import { TooltipProvider } from "@/components/ui/tooltip";
import { parseCitation } from "@/lib/citations";
import { isDark, subscribeDark } from "@/lib/theme";
import { copyText } from "@/lib/utils";

import { parseProperties, type PropertyValue } from "./frontmatter";
import { mathFieldFocused } from "./liveFocus";
import { MATH_ARRAYSTRETCH, mathLiveReady, noteMathPress, staticMath } from "./mathField";
import { ancestorAt } from "./syntax";

/**
 * The rendered stand-ins Live mode draws over markdown source. Each one, when
 * pressed, puts the caret `caret` characters into the source it replaces, so
 * the source reveals and the press becomes an edit.
 */
abstract class SourceWidget extends WidgetType {
  constructor(readonly caret: number) {
    super();
  }

  protected reveal(dom: HTMLElement, view: EditorView) {
    dom.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      const at = view.posAtDOM(dom) + this.caret;
      view.dispatch({ selection: { anchor: Math.min(at, view.state.doc.length) } });
      view.focus();
    });
  }
}

/** Rendered KaTeX by display flag and source; a note re-renders often. */
const katexCache = new Map<string, string | null>();
const KATEX_CACHE_MAX = 500;

/** `\left[ \begin{array}…\end{array} \right]` drawn like `bmatrix`: an
 *  array keeps `\arraycolsep` outside its first and last columns (matrices
 *  drop it), which reads as a gap inside the brackets. Render-time only, and
 *  the field draws it the same (`patchArrays` in `mathField.ts`). */
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
 *  `MATH_ARRAYSTRETCH`, as the visual field draws them (`mathField.ts`). */
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
 * (`livePreview.ts`), so the selection layer highlights inline maths like a
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

export class ImageWidget extends SourceWidget {
  constructor(
    readonly src: string,
    readonly alt: string,
    readonly block: boolean,
    caret: number,
  ) {
    super(caret);
  }

  eq(other: ImageWidget) {
    return (
      other.src === this.src &&
      other.alt === this.alt &&
      other.block === this.block &&
      other.caret === this.caret
    );
  }

  toDOM(view: EditorView) {
    const dom = document.createElement(this.block ? "div" : "span");
    dom.className = this.block ? "cm-image cm-image-block" : "cm-image";
    if (!this.src) {
      dom.textContent = this.alt || "image";
      dom.classList.add("cm-image-missing");
    } else {
      const img = document.createElement("img");
      img.src = this.src;
      img.alt = this.alt;
      img.draggable = false;
      // The picture's height arrives after the line was measured.
      img.addEventListener("load", () => view.requestMeasure());
      img.addEventListener("error", () => {
        img.remove();
        dom.textContent = this.alt || "image";
        dom.classList.add("cm-image-missing");
        view.requestMeasure();
      });
      dom.appendChild(img);
    }
    this.reveal(dom, view);
    return dom;
  }

  get estimatedHeight() {
    return this.block ? 240 : -1;
  }
}

/** Drawn diagrams by theme and source, so a rebuilt widget paints at once. */
const diagramCache = new Map<string, Diagram | null>();
const DIAGRAM_CACHE_MAX = 100;

/** Debounce while the fence is being typed, so it lays out once per pause. */
const DIAGRAM_SETTLE_MS = 250;

/** What a `MermaidWidget`'s DOM is showing, kept across `updateDOM`. */
class DiagramPainter {
  private timer: number | undefined;
  private run = 0;
  private stopWatching: () => void;
  private drawn = false;

  constructor(
    readonly dom: HTMLElement,
    private view: EditorView,
    public code: string,
    private editing: boolean,
  ) {
    // A theme flip re-renders: mermaid bakes the palette into the SVG.
    this.stopWatching = subscribeDark(() => this.paint(this.code, false));
  }

  paint(code: string, settle: boolean) {
    this.code = code;
    window.clearTimeout(this.timer);
    const key = `${isDark() ? "D" : "L"}${code}`;
    const hit = diagramCache.get(key);
    if (hit !== undefined) return this.show(hit);
    if (!this.drawn) this.placeholder(false);
    const run = ++this.run;
    this.timer = window.setTimeout(async () => {
      let out: Diagram | null = null;
      try {
        // A fresh id each time: mermaid deletes any element already holding it.
        out = await renderMermaid(code, mermaidId());
      } catch {
        // Past parsing; drawn the same as source that does not parse.
      }
      if (run !== this.run) return;
      if (diagramCache.size >= DIAGRAM_CACHE_MAX) diagramCache.clear();
      diagramCache.set(key, out);
      this.show(out);
    }, settle ? DIAGRAM_SETTLE_MS : 0);
  }

  private show(diagram: Diagram | null) {
    if (!diagram) {
      // Mid-edit the last good drawing stays; otherwise the source shows, so
      // the mistake is findable.
      if (!this.editing || !this.drawn) this.placeholder(true);
      return;
    }
    const box = document.createElement("div");
    box.className = "diagram";
    if (diagram.size) {
      for (const [name, value] of Object.entries(diagramBounds(diagram.size))) box.style.setProperty(name, value);
    }
    box.innerHTML = diagram.svg;
    this.dom.replaceChildren(box);
    this.dom.classList.remove("cm-mermaid-error");
    this.drawn = true;
    this.view.requestMeasure();
  }

  /** The source, muted while it renders or red when it does not parse; empty
   *  under a fence being edited, which already shows it. */
  private placeholder(failed: boolean) {
    this.drawn = false;
    this.dom.classList.toggle("cm-mermaid-error", failed && !this.editing);
    if (this.editing) this.dom.replaceChildren();
    else {
      const pre = document.createElement("pre");
      pre.className = "cm-mermaid-source";
      pre.textContent = this.code.trim() || "Empty diagram";
      this.dom.replaceChildren(pre);
    }
    this.view.requestMeasure();
  }

  destroy() {
    this.run++;
    window.clearTimeout(this.timer);
    this.stopWatching();
  }
}

const painters = new WeakMap<HTMLElement, DiagramPainter>();

/** A ```mermaid fence, drawn. `editing` is the copy shown under a fence whose
 *  source the selection touches: it keeps its last drawing while the source
 *  does not parse, and a press does not move the caret. */
export class MermaidWidget extends SourceWidget {
  constructor(
    readonly code: string,
    readonly editing: boolean,
    caret: number,
  ) {
    super(caret);
  }

  eq(other: MermaidWidget) {
    return other.code === this.code && other.editing === this.editing && other.caret === this.caret;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("div");
    dom.className = this.editing ? "cm-mermaid cm-mermaid-editing" : "cm-mermaid";
    const painter = new DiagramPainter(dom, view, this.code, this.editing);
    painters.set(dom, painter);
    painter.paint(this.code, false);
    if (!this.editing) this.reveal(dom, view);
    return dom;
  }

  /** Typing in the fence redraws in place instead of rebuilding the DOM. */
  updateDOM(dom: HTMLElement, _view: EditorView, from: this) {
    const painter = painters.get(dom);
    if (!painter || from.editing !== this.editing || from.caret !== this.caret) return false;
    if (painter.code !== this.code) painter.paint(this.code, true);
    return true;
  }

  destroy(dom: HTMLElement) {
    painters.get(dom)?.destroy();
  }

  get estimatedHeight() {
    return 240;
  }
}

/** A bullet list's `-`, `*` or `+`. */
export class BulletWidget extends SourceWidget {
  constructor() {
    super(0);
  }

  eq() {
    return true;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("span");
    dom.className = "cm-list-bullet";
    dom.textContent = "•";
    this.reveal(dom, view);
    return dom;
  }
}

/** A task's `[ ]` / `[x]`. Pressing it flips the character between the
 *  brackets — an ordinary, undoable edit — and leaves the caret alone. */
export class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }

  eq(other: CheckboxWidget) {
    return other.checked === this.checked;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("span");
    dom.className = "cm-task-box";
    dom.setAttribute("role", "checkbox");
    dom.setAttribute("aria-checked", String(this.checked));
    dom.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      const at = view.posAtDOM(dom) + 1;
      const now = view.state.sliceDoc(at, at + 1);
      view.dispatch({ changes: { from: at, to: at + 1, insert: /x/i.test(now) ? " " : "x" } });
    });
    return dom;
  }
}

/** Each chip's React root, unmounted with its DOM. */
const chipRoots = new WeakMap<HTMLElement, Root>();

/** An inline code span that is wholly a citation (`mentionSyntax.ts`), as the
 *  chat's chip: `CitationCode` in a React root of its own. A click opens the
 *  file (⌘ in a new tab); a press beside it places the caret as usual. */
export class CitationWidget extends WidgetType {
  constructor(readonly text: string) {
    super();
  }

  eq(other: CitationWidget) {
    return other.text === this.text;
  }

  toDOM() {
    const dom = document.createElement("span");
    dom.className = "cm-citation";
    // A press must neither move the caret, which would reveal the source and
    // drop the chip mid-click, nor take focus; the click still opens it.
    dom.addEventListener("mousedown", (e) => {
      if (e.button === 0) e.preventDefault();
    });
    const shape = parseCitation(this.text);
    // Until a tail resolves, and for good if it never does: the span as Live
    // mode draws any other inline code.
    const code = createElement("span", { className: "cm-inline-code" }, this.text);
    const root = createRoot(dom);
    // Outside `AppLayout`'s tree, so the lightbox's tooltips need a provider.
    const children = shape ? createElement(CitationCode, { shape, code }) : code;
    root.render(createElement(TooltipProvider, { delayDuration: 500, children }));
    chipRoots.set(dom, root);
    return dom;
  }

  destroy(dom: HTMLElement) {
    const root = chipRoots.get(dom);
    chipRoots.delete(dom);
    // Deferred: CodeMirror may destroy widgets inside a React commit.
    if (root) queueMicrotask(() => root.unmount());
  }

  /** Every event inside belongs to the chip, none to the editor. */
  ignoreEvent() {
    return true;
  }
}

/** What Live mode draws over a code block's opening fence: the language,
 *  which reveals the fence for retagging, and a copy button. */
export class CodeHeaderWidget extends WidgetType {
  constructor(
    readonly label: string,
    readonly auto: boolean,
    readonly code: string,
  ) {
    super();
  }

  eq(other: CodeHeaderWidget) {
    return other.label === this.label && other.auto === this.auto && other.code === this.code;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("span");
    dom.className = "cm-code-header";

    const label = document.createElement("span");
    label.className = "cm-code-label";
    label.textContent = this.label;
    label.title = this.auto ? "Detected automatically — click to set the language" : "Click to set the language";
    label.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      // The end of the fence line, where the language is typed.
      const at = view.state.doc.lineAt(view.posAtDOM(dom)).to;
      view.dispatch({ selection: { anchor: at } });
      view.focus();
    });

    const copy = document.createElement("button");
    copy.type = "button";
    copy.className = "cm-code-copy";
    copy.textContent = "Copy";
    let timer: number | undefined;
    copy.addEventListener("mousedown", (e) => e.preventDefault());
    copy.addEventListener("click", () => {
      void copyText(this.code).then((ok) => {
        if (!ok) return;
        copy.textContent = "Copied";
        window.clearTimeout(timer);
        timer = window.setTimeout(() => (copy.textContent = "Copy"), 1200);
      });
    });

    dom.append(label, copy);
    return dom;
  }
}

/** A thematic break (`---`). */
export class RuleWidget extends SourceWidget {
  constructor() {
    super(0);
  }

  eq() {
    return true;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("div");
    dom.className = "cm-hr";
    this.reveal(dom, view);
    return dom;
  }

  get estimatedHeight() {
    return 24;
  }
}

/** Digits-led values (versions, dates, counts) hold a column. */
const NUMERIC = /^[-+]?\d[\d.,:_/-]*$/;

function propertyValue(value: PropertyValue): HTMLElement {
  const dom = document.createElement("div");
  dom.className = "cm-prop-value";
  if (value.kind === "list") {
    for (const item of value.items) {
      const chip = document.createElement("span");
      chip.className = "cm-prop-chip";
      chip.textContent = item;
      dom.appendChild(chip);
    }
  } else {
    dom.textContent = value.text;
    if (value.kind === "text" && NUMERIC.test(value.text)) dom.classList.add("cm-prop-number");
  }
  return dom;
}

/** YAML frontmatter as a properties card. Pressing a row reveals the source
 *  with the caret on that row's line. */
export class PropertiesWidget extends WidgetType {
  constructor(readonly source: string) {
    super();
  }

  eq(other: PropertiesWidget) {
    return other.source === this.source;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("div");
    dom.className = "cm-props";
    const card = document.createElement("div");
    card.className = "cm-props-card";
    const label = document.createElement("div");
    label.className = "cm-props-label";
    label.textContent = "Properties";
    const grid = document.createElement("div");
    grid.className = "cm-props-grid";
    for (const prop of parseProperties(this.source)) {
      const at = String(prop.at);
      if (prop.key == null) {
        const raw = document.createElement("div");
        raw.className = "cm-prop-raw";
        raw.textContent = prop.value.kind === "list" ? prop.value.items.join(", ") : prop.value.text;
        raw.dataset.at = at;
        grid.appendChild(raw);
        continue;
      }
      const key = document.createElement("div");
      key.className = "cm-prop-key";
      key.textContent = prop.key;
      key.title = prop.key;
      key.dataset.at = at;
      const value = propertyValue(prop.value);
      value.dataset.at = at;
      grid.append(key, value);
    }
    card.append(label, grid);
    dom.appendChild(card);

    dom.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      const row = (e.target as Element).closest<HTMLElement>("[data-at]");
      // Off a row, the first line inside the fences.
      const offset = row ? Number(row.dataset.at) : this.source.indexOf("\n") + 1;
      const at = view.posAtDOM(dom) + offset;
      view.dispatch({ selection: { anchor: Math.min(at, view.state.doc.length) } });
      view.focus();
    });
    return dom;
  }

  get estimatedHeight() {
    return 40 + Math.max(0, this.source.split("\n").length - 2) * 24;
  }
}
