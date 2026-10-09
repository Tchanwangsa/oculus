import { WidgetType, type EditorView } from "@codemirror/view";

import { FieldController } from "./controller/field-controller";

const controllers = new WeakMap<HTMLElement, FieldController>();

/** The field in place of a maths node. Keeps its DOM across the doc changes
 *  its own typing causes (`updateDOM`), or MathLive would lose its caret. */
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
    const field = new FieldController(view, this.source, this.display, this.block, this.id);
    controllers.set(field.dom, field);
    return field.dom;
  }

  /** Reused only for the same maths: another maths gets a field of its own. */
  updateDOM(dom: HTMLElement) {
    const field = controllers.get(dom);
    if (!field || field.display !== this.display || field.block !== this.block || field.id !== this.id) return false;
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
