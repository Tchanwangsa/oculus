import { NOT_MATHS_PRESS } from "./formula";

/** A third prose click takes the DOM paragraph, including inline maths.
 *  WebKit's native paragraph selection can stop at an inline math box. */
export function selectParagraph(target: EventTarget | null): boolean {
  if (!(target instanceof Element) || target.closest(`${NOT_MATHS_PRESS}, .cm-editor`)) return false;
  const paragraph = target.closest("p");
  if (!paragraph?.closest("[data-selectable]") || target.closest(".katex, .katex-display")) return false;
  const selection = window.getSelection();
  if (!selection) return false;
  const range = document.createRange();
  range.selectNodeContents(paragraph);
  selection.removeAllRanges();
  selection.addRange(range);
  return true;
}
