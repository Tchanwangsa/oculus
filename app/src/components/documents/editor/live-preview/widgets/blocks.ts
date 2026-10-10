import { WidgetType, type EditorView } from "@codemirror/view";

import { copyText } from "@/lib/utils";

import { SourceWidget } from "./source";

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
