import { WidgetType, type EditorView } from "@codemirror/view";

import { openRustField } from "../rustField/controller";
import type { VisualField } from "./registry";

const controllers = new WeakMap<HTMLElement, VisualField>();

/** The field in place of a maths node. Keeps its DOM across the doc changes
 *  its own typing causes (`updateDOM`), or the field would lose its caret. */
export class MathFieldWidget extends WidgetType {
  constructor(
    readonly source: string,
    readonly display: boolean,
    readonly block: boolean,
    readonly id: number,
  ) {
    super();
  }

  eq(other: MathFieldWidget) {
    return (
      other.source === this.source && other.display === this.display && other.block === this.block && other.id === this.id
    );
  }

  toDOM(view: EditorView) {
    const field = openRustField(view, this.source, this.display, this.block, this.id);
    if (!field) return this.unopened();
    controllers.set(field.dom, field);
    return field.dom;
  }

  /** The source, muted, while maths the field couldn't open drops to TeX
   *  mode. */
  private unopened(): HTMLElement {
    const dom = document.createElement(this.block ? "div" : "span");
    dom.className = "cm-math-pending";
    dom.textContent = this.source;
    return dom;
  }

  /** Reused only for the same maths: another maths gets a field of its own.
   *  An unopened field stays so until TeX mode replaces it. */
  updateDOM(dom: HTMLElement) {
    const field = controllers.get(dom);
    if (!field) return dom.classList.contains("cm-math-pending");
    if (field.display !== this.display || field.block !== this.block || field.id !== this.id) return false;
    field.sync(this.source);
    return true;
  }

  destroy(dom: HTMLElement) {
    controllers.get(dom)?.destroy();
  }

  ignoreEvent() {
    return true;
  }

  get estimatedHeight() {
    return this.block ? 56 : -1;
  }
}
