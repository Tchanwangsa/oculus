import { bands, frameOrigin, paintBands, readLayout } from "@/lib/maths/geometry";
import { fontAscent } from "@/lib/maths/geometry/font";
import { fieldOf, formulaOf, type Formula } from "./formula";
import { dropLayer, openLayer } from "./session";
import { proseBands, type ProsePiece } from "./proseGeometry";

const LAYER = { attribute: "data-math-prose-selecting", className: "md-prose-bands" };
const ACTIVE = "data-math-prose-highlight";
const SKIP = '.katex, .md-prose-bands, button, [role="button"], [role="tab"], [data-copy-skip], [aria-hidden="true"]';

/** One paragraph’s native selection drawn as uniform visual lines. Inline
 *  arrays with several rows keep their structural maths bands instead. */
export class ProseBands {
  readonly layer: HTMLElement;
  range: Range;
  #resize: ResizeObserver;
  #fonts = () => this.redraw();

  constructor(readonly root: HTMLElement, range: Range) {
    this.range = range.cloneRange();
    this.layer = openLayer(root, LAYER);
    root.setAttribute(ACTIVE, "");
    this.redraw();
    this.#resize = new ResizeObserver(() => this.redraw());
    this.#resize.observe(root);
    document.fonts.addEventListener("loadingdone", this.#fonts);
  }

  static accepts(root: HTMLElement, range: Range): boolean {
    const formulas = selectedFormulas(root, range);
    return formulas.length > 0 && formulas.every(formula => {
      const slots = fieldOf(formula)?.slots;
      return slots && slots.filter(s => s.parent == null).length <= 1 &&
        new Set(slots.filter(s => s.kind === "cell").map(s => s.row)).size <= 1;
    });
  }

  update(range: Range) {
    this.range = range.cloneRange();
    this.redraw();
  }

  redraw() {
    if (!this.root.isConnected) return;
    const pieces: ProsePiece[] = [];
    const origin = frameOrigin(this.layer);
    const walker = document.createTreeWalker(this.root, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (node.parentElement?.closest(SKIP) || !this.range.intersectsNode(node)) continue;
      const part = document.createRange();
      part.selectNodeContents(node);
      if (node === this.range.startContainer) part.setStart(node, this.range.startOffset);
      if (node === this.range.endContainer) part.setEnd(node, this.range.endOffset);
      const ascent = fontAscent(getComputedStyle(node.parentElement!));
      for (const r of part.getClientRects()) {
        if (r.width <= 0 || r.height <= 0) continue;
        pieces.push({ box: { left: r.left - origin.x, right: r.right - origin.x, top: r.top - origin.y, bottom: r.bottom - origin.y }, baseline: r.top - origin.y + ascent });
      }
    }
    for (const formula of selectedFormulas(this.root, this.range)) {
      const layout = readLayout(formula.html, this.layer);
      for (const box of bands(layout, 0, Number.MAX_SAFE_INTEGER, fieldOf(formula)?.slots)) {
        const atoms = layout.items.filter(item => item.ink.left >= box.left - 1 && item.ink.right <= box.right + 1 && item.ink.top < box.bottom && item.ink.bottom > box.top);
        atoms.sort((a, b) => b.size - a.size || (b.to - b.from) - (a.to - a.from));
        if (atoms[0]) pieces.push({ box, baseline: atoms[0].baseline });
      }
    }
    paintBands(this.layer, proseBands(pieces), "md-math-band", 0);
  }

  close() {
    this.#resize.disconnect();
    document.fonts.removeEventListener("loadingdone", this.#fonts);
    this.root.removeAttribute(ACTIVE);
    dropLayer(this.layer, this.root, undefined, LAYER);
  }
}

function selectedFormulas(root: HTMLElement, range: Range): Formula[] {
  return Array.from(root.querySelectorAll(".katex")).filter(el =>
    !el.closest(".katex-display") && range.intersectsNode(el),
  ).flatMap(el => {
    const formula = formulaOf(el);
    return formula ? [formula] : [];
  });
}
