import { type MathField, MathsTrap } from "@/lib/maths";
import { bands, framePoint, measure, paintBands, readLayout, stopAt, type Layout, type Measured } from "@/lib/maths/geometry";
import type { Formula } from "./formula";

/** A field's top-level rows: a display's lines, each its own band. */
const topRows = (field: MathField) => field.slots.filter((s) => s.parent == null);

/** Marks the formula a selection is drawn in (`styles/math.css`). */
const SELECTING = "data-math-selecting";

/**
 * A selection in one rendered formula: its edit model and the band layer,
 * an absolutely positioned zero-size span inside the formula's `.katex`,
 * whose own corner is the frame every box is read in, so an inline formula
 * broken over lines still lines up. Nothing in the formula's flow changes.
 */
export class Session {
  readonly layer: HTMLElement;
  /** The model with the current selection; `base` until the first one. */
  field: MathField;
  /** The stop the last plain press landed on, which Shift extends from. */
  anchor = 0;
  #layout: Layout = { items: [] };
  #measured: Measured = { x: new Float64Array(), top: new Float64Array(), bottom: new Float64Array() };
  #resize: ResizeObserver;

  constructor(
    readonly formula: Formula,
    /** The formula's model as opened (`fieldOf`), never freed here. */
    readonly base: MathField,
  ) {
    this.field = base;
    this.layer = document.createElement("span");
    this.layer.className = "md-math-bands";
    this.layer.setAttribute("aria-hidden", "true");
    formula.root.setAttribute(SELECTING, "");
    formula.root.append(this.layer);
    this.#relayout();
    // Fonts arriving or the column resizing move the glyphs.
    this.#resize = new ResizeObserver(() => this.redraw());
    this.#resize.observe(formula.html);
  }

  get connected(): boolean {
    return this.formula.root.isConnected;
  }

  /** The stop a pointer at this viewport point lands on. */
  stopAt(clientX: number, clientY: number): number | null {
    const { x, y } = framePoint(this.layer, clientX, clientY);
    return stopAt(this.#layout, this.base, this.#measured, x, y);
  }

  /** Selects between two stops, widened by the model over whole
   *  structures; false when the engine trapped. */
  select(anchor: number, head: number): boolean {
    return this.#take(() => this.base.select(anchor, head));
  }

  /** The model's select-all from the current selection. */
  selectAll(): boolean {
    return this.#take(() => this.field.run("selectAll").field);
  }

  get empty(): boolean {
    const [from, to] = this.field.selected;
    return to <= from;
  }

  /** Re-reads the boxes and redraws the bands. */
  redraw() {
    if (!this.connected) return;
    this.#relayout();
    this.#draw();
  }

  close() {
    this.#resize.disconnect();
    this.layer.remove();
    this.formula.root.removeAttribute(SELECTING);
    if (this.field !== this.base) this.field.free();
    this.field = this.base;
  }

  #take(next: () => MathField): boolean {
    let field: MathField;
    try {
      field = next();
    } catch (e) {
      if (e instanceof MathsTrap) return false;
      throw e;
    }
    if (this.field !== this.base) this.field.free();
    this.field = field;
    this.#draw();
    return true;
  }

  #relayout() {
    this.#layout = readLayout(this.formula.html, this.layer);
    this.#measured = measure(this.#layout, this.base);
  }

  #draw() {
    const [from, to] = this.field.selected;
    paintBands(this.layer, bands(this.#layout, from, to, topRows(this.base)), "md-math-band");
  }
}
