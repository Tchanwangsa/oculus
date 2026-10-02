import { WidgetType, type EditorView } from "@codemirror/view";
import katex from "katex";

import "katex/dist/katex.min.css";

import { diagramBounds, mermaidId, renderMermaid, type Diagram } from "@/components/markdown/mermaidRender";
import { isDark, subscribeDark } from "@/lib/theme";
import { copyText } from "@/lib/utils";

import { parseProperties, type PropertyValue } from "./frontmatter";

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

/** KaTeX HTML for `source`, or null if it does not parse. */
function renderMath(source: string, display: boolean): string | null {
  const key = `${display ? "D" : "I"}${source}`;
  const hit = katexCache.get(key);
  if (hit !== undefined) return hit;
  let html: string | null;
  try {
    html = katex.renderToString(source, { displayMode: display, throwOnError: true });
  } catch {
    html = null;
  }
  if (katexCache.size >= KATEX_CACHE_MAX) katexCache.clear();
  katexCache.set(key, html);
  return html;
}

export class MathWidget extends SourceWidget {
  constructor(
    readonly source: string,
    readonly display: boolean,
    caret: number,
  ) {
    super(caret);
  }

  eq(other: MathWidget) {
    return other.source === this.source && other.display === this.display && other.caret === this.caret;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement(this.display ? "div" : "span");
    dom.className = this.display ? "cm-math cm-math-display" : "cm-math";
    const html = this.source.trim() ? renderMath(this.source, this.display) : null;
    if (html) {
      dom.innerHTML = html;
    } else {
      // Bad or empty LaTeX shows its source, so the mistake is findable.
      dom.classList.add("cm-math-error");
      dom.textContent = this.source.trim() ? this.source : "Empty equation";
    }
    this.reveal(dom, view);
    return dom;
  }

  get estimatedHeight() {
    return this.display ? 56 : -1;
  }
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
