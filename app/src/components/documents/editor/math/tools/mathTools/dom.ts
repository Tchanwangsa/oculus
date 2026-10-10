import { completionStatus } from "@codemirror/autocomplete";
import type { EditorState } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import katex from "katex";

import { mathFieldFocused } from "@/components/documents/editor/core/liveFocus";
import { previewOf, type MathEntry } from "../mathPalette";
import { sourceOf } from "./insert";

/** Button and completion previews, by LaTeX; the set is small and fixed. */
const previewCache = new Map<string, string>();

export function previewHtml(latex: string): string {
  let html = previewCache.get(latex);
  if (html === undefined) {
    html = katex.renderToString(latex, { throwOnError: false });
    previewCache.set(latex, html);
  }
  return html;
}

export function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text?: string) {
  const node = document.createElement(tag);
  node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

/** A strip one row tall that a mouse wheel scrolls sideways. */
export function sideways(strip: HTMLElement) {
  strip.addEventListener(
    "wheel",
    (e) => {
      if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
      e.preventDefault();
      strip.scrollLeft += e.deltaY;
    },
    { passive: false },
  );
}

export const CLOSE_ICON =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>';

/** A control that never takes focus: the root's mousedown is cancelled, so
 *  the editor keeps focus and its selection. */
export function button(className: string, label: string, onClick: () => void): HTMLButtonElement {
  const b = el("button", className);
  b.type = "button";
  b.tabIndex = -1;
  b.setAttribute("aria-label", label);
  b.title = label;
  b.addEventListener("click", onClick);
  return b;
}

/** A palette cell: the entry's KaTeX preview, inserting it on click. */
export function cellButton(entry: MathEntry, onClick: () => void): HTMLButtonElement {
  const b = button(entry.wide ? "cm-math-cell cm-math-cell-wide" : "cm-math-cell", sourceOf(entry.template), onClick);
  b.innerHTML = previewHtml(previewOf(entry));
  return b;
}

/** Hidden while neither the editor nor its field has focus, and while the
 *  completion list is up. True when it has just come back. */
export function syncHidden(view: EditorView, dom: HTMLElement, state: EditorState): boolean {
  const hide = !(view.hasFocus || mathFieldFocused()) || completionStatus(state) === "active";
  const was = dom.classList.contains("cm-math-tools-hidden");
  dom.classList.toggle("cm-math-tools-hidden", hide);
  return was && !hide;
}
