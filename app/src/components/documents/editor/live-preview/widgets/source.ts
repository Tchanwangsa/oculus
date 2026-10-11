import { WidgetType, type EditorView } from "@codemirror/view";

/**
 * The rendered stand-ins Live mode draws over markdown source. Each one, when
 * pressed, puts the caret `caret` characters into the source it replaces, so
 * the source reveals and the press becomes an edit.
 */
export abstract class SourceWidget extends WidgetType {
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
