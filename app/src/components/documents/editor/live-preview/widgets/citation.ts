import { WidgetType } from "@codemirror/view";
import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";

import { CitationCode } from "@/components/markdown/Citation";
import { TooltipProvider } from "@/components/ui/tooltip";
import { parseCitation } from "@/lib/citations";

/** Each chip's React root, unmounted with its DOM. */
const chipRoots = new WeakMap<HTMLElement, Root>();

/** An inline code span that is wholly a citation (`mentionSyntax.ts`), as the
 *  chat's chip: `CitationCode` in a React root of its own. A click opens the
 *  file (⌘ in a new tab); a press beside it places the caret as usual. */
export class CitationWidget extends WidgetType {
  constructor(readonly text: string) {
    super();
  }

  eq(other: CitationWidget) {
    return other.text === this.text;
  }

  toDOM() {
    const dom = document.createElement("span");
    dom.className = "cm-citation";
    // A press must neither move the caret, which would reveal the source and
    // drop the chip mid-click, nor take focus; the click still opens it.
    dom.addEventListener("mousedown", (e) => {
      if (e.button === 0) e.preventDefault();
    });
    const shape = parseCitation(this.text);
    // Until a tail resolves, and for good if it never does: the span as Live
    // mode draws any other inline code.
    const code = createElement("span", { className: "cm-inline-code" }, this.text);
    const root = createRoot(dom);
    // Outside `AppLayout`'s tree, so the lightbox's tooltips need a provider.
    const children = shape ? createElement(CitationCode, { shape, code }) : code;
    root.render(createElement(TooltipProvider, { delayDuration: 500, children }));
    chipRoots.set(dom, root);
    return dom;
  }

  destroy(dom: HTMLElement) {
    const root = chipRoots.get(dom);
    chipRoots.delete(dom);
    // Deferred: CodeMirror may destroy widgets inside a React commit.
    if (root) queueMicrotask(() => root.unmount());
  }

  /** Every event inside belongs to the chip, none to the editor. */
  ignoreEvent() {
    return true;
  }
}
