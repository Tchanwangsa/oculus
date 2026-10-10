import type { EditorView } from "@codemirror/view";

import { diagramBounds, mermaidId, renderMermaid, type Diagram } from "@/components/markdown/mermaidRender";
import { isDark, subscribeDark } from "@/lib/ui/theme";

import { SourceWidget } from "./source";

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
