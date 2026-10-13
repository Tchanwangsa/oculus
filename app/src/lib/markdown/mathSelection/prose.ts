import { paintBands } from "@/lib/maths/geometry";
import { hasInlineMath, selectionLines } from "./lines";
import { dropLayer, openLayer } from "./session";

const LAYER = { attribute: "data-math-prose-selecting", className: "md-prose-bands" };
const ACTIVE = "data-math-prose-highlight";

/** A native text-flow selection using the same line geometry as the note editor. */
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
    return hasInlineMath(root, range);
  }

  update(range: Range) {
    this.range = range.cloneRange();
    this.redraw();
  }

  redraw() {
    if (!this.root.isConnected) return;
    paintBands(this.layer, selectionLines(this.root, this.range, this.layer) ?? [], "md-math-band", 0);
  }

  close() {
    this.#resize.disconnect();
    document.fonts.removeEventListener("loadingdone", this.#fonts);
    this.root.removeAttribute(ACTIVE);
    dropLayer(this.layer, this.root, undefined, LAYER);
  }
}
