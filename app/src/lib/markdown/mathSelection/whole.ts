import { ProseBands } from "./prose";
import { bands, paintBands, readLayout, type BandSlot } from "@/lib/maths/geometry";
import { mathAround } from "../selection";
import { fieldOf, formulaOf, type Formula } from "./formula";
import { dropLayer, openLayer } from "./session";
import { watchLayout } from "./resize";

/**
 * A rendered formula a selection takes whole — a text selection over it in
 * a chat reply or a file, a note's selection covering its rendering — drawn
 * as the in-place selection draws all of it: bands over its atoms
 * (`bands`), never a box over the formula.
 */
export class WholeBands {
  readonly layer: HTMLElement;
  #unwatch: () => void;

  constructor(
    readonly formula: Formula,
    /** The slots of the model over the offsets its source map indexes. */
    readonly slots: readonly BandSlot[],
  ) {
    this.layer = openLayer(formula.root);
    this.redraw();
    this.#unwatch = watchLayout(formula, () => this.redraw());
  }

  redraw() {
    if (!this.formula.root.isConnected) return;
    const layout = readLayout(this.formula.html, this.layer);
    // Inline prose already supplies the line’s height; extra pad makes bumps.
    const pad = this.formula.display ? undefined : 0;
    paintBands(this.layer, bands(layout, 0, Number.MAX_SAFE_INTEGER, this.slots), "md-math-band", pad);
  }

  close() {
    this.#unwatch();
    dropLayer(this.layer, this.formula.root);
  }
}

/** Bands over a chat or file formula, its slots from its own model
 *  (`fieldOf`); null for maths drawn without the source map. */
function wholeBands(math: Element): WholeBands | null {
  const formula = formulaOf(math);
  return formula && new WholeBands(formula, fieldOf(formula)?.slots ?? []);
}

function touches(node: Node, range: Range): boolean {
  try {
    return range.intersectsNode(node);
  } catch {
    return false;
  }
}

/**
 * A text selection in rendered markdown takes each formula it touches
 * whole (`selectionMarkdown`), and paints it so: `WholeBands` on each, as
 * the native highlight leaves gaps between KaTeX's glyph boxes and is
 * turned off on them (`styles/math.css`). Installed once, app-wide.
 */
export function watchMathSelection() {
  const painted = new Map<Element, WholeBands | null>();
  const prose = new Map<HTMLElement, ProseBands>();
  document.addEventListener("selectionchange", () => {
    const selection = window.getSelection();
    const next = new Set<Element>();
    let range: Range | null = null;
    if (selection && selection.rangeCount && !selection.isCollapsed) {
      range = selection.getRangeAt(0);
      const inside = mathAround(range.commonAncestorContainer);
      if (inside) next.add(inside);
      else {
        const root = range.commonAncestorContainer;
        const scope = root.nodeType === Node.ELEMENT_NODE ? (root as Element) : root.parentElement;
        for (const el of Array.from(scope?.querySelectorAll(".katex-display, .katex") ?? [])) {
          if (mathAround(el) === el && touches(el, range)) next.add(el);
        }
      }
    }
    const paragraphs = new Set<HTMLElement>();
    if (range && !mathAround(range.commonAncestorContainer)) {
      for (const el of next) {
        const p = el.closest("p");
        if (p instanceof HTMLElement && !p.closest('.cm-editor, [contenteditable="true"]') && ProseBands.accepts(p, range)) paragraphs.add(p);
      }
    }
    for (const [p, layer] of prose) {
      if (paragraphs.has(p) && p.isConnected) continue;
      layer.close();
      prose.delete(p);
    }
    for (const p of paragraphs) {
      const layer = prose.get(p);
      if (layer) layer.update(range!);
      else prose.set(p, new ProseBands(p, range!));
      for (const el of next) if (!el.classList.contains("katex-display") && p.contains(el)) next.delete(el);
    }
    for (const [el, bands] of painted) {
      if (next.has(el) && el.isConnected) continue;
      bands?.close();
      painted.delete(el);
    }
    for (const el of next) if (!painted.has(el)) painted.set(el, wholeBands(el));
  });
}
