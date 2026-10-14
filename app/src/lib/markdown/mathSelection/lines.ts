import { bands, frameOrigin, readLayout, type Box } from "@/lib/maths/geometry";
import { fontAscent } from "@/lib/maths/geometry/font";
import { lineBands, type LinePiece } from "@/lib/maths/geometry/lineBands";
import { formulaOf, type Formula } from "./formula";

const SKIP = '.katex, .katex-display, .md-prose-bands, .cm-math-view, .cm-math-display, .cm-widgetBuffer, button, [role="button"], [role="tab"], [data-copy-skip], [aria-hidden="true"]';

/** Every inline formula touched by a text range, or null when a drawing
 *  cannot supply mapped geometry. A prose selection takes each one whole. */
function inlineFormulas(root: HTMLElement, range: Range): Formula[] | null {
  const out: Formula[] = [];
  const editor = root.closest(".cm-editor");
  for (const el of root.querySelectorAll(".katex")) {
    if (el.closest(".cm-editor") !== editor || el.closest(".katex-display, .cm-math-view") || !range.intersectsNode(el)) continue;
    const formula = formulaOf(el);
    if (!formula || !formula.html.querySelector(":scope > .katex-base > .katex-strut")) return null;
    out.push(formula);
  }
  return out;
}

export function hasInlineMath(root: HTMLElement, range: Range): boolean {
  return (inlineFormulas(root, range)?.length ?? 0) > 0;
}

/** Shared text-and-maths selection geometry for native rendered paragraphs
 *  and editable note lines. Hosts supply the DOM range and overlay frame;
 *  the tallest selected ink sets each visual line’s highlight height. */
export function selectionLines(root: HTMLElement, range: Range, frame: Element): Box[] | null {
  const formulas = inlineFormulas(root, range);
  if (!formulas?.length) return null;
  const pieces: LinePiece[] = [];
  const origin = frameOrigin(frame);
  const editor = root.closest(".cm-editor");
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (node.parentElement?.closest(".cm-editor") !== editor || node.parentElement?.closest(SKIP) || !range.intersectsNode(node)) continue;
    const part = document.createRange();
    part.selectNodeContents(node);
    if (node === range.startContainer) part.setStart(node, range.startOffset);
    if (node === range.endContainer) part.setEnd(node, range.endOffset);
    const ascent = fontAscent(getComputedStyle(node.parentElement!));
    for (const r of part.getClientRects()) {
      if (r.width <= 0 || r.height <= 0) continue;
      pieces.push({ box: { left: r.left - origin.x, right: r.right - origin.x, top: r.top - origin.y, bottom: r.bottom - origin.y }, baseline: r.top - origin.y + ascent });
    }
  }
  for (const formula of formulas) {
    for (const base of formula.html.querySelectorAll(":scope > .katex-base")) {
      const strut = base.querySelector(":scope > .katex-strut");
      if (!strut) continue;
      // The base’s strut ends its depth below the actual line baseline.
      // Matrix cells and tall delimiters have baselines of their own.
      const baseline = strut.getBoundingClientRect().bottom - origin.y +
        (parseFloat(getComputedStyle(strut).verticalAlign) || 0);
      const layout = readLayout(base, frame);
      // Include the selected base’s operator spacing as well as its ink.
      const bounds = base.getBoundingClientRect();
      for (const box of bands(layout, 0, Number.MAX_SAFE_INTEGER)) {
        pieces.push({ box: { ...box, left: Math.min(box.left, bounds.left - origin.x), right: Math.max(box.right, bounds.right - origin.x) }, baseline });
      }
    }
  }
  return lineBands(pieces);
}
