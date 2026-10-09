import type { ClipboardEvent, DragEvent } from "react";
import { elementMarkdown } from "@/lib/markdown/selection";
import { markdownOfSelection, parsedDoc, type PageSelection } from "./parsedDoc";

/** A text layer's text: its spans' text, a `<br>` as a newline. */
function layerText(layer: Element): { raw: string; nodes: Text[]; starts: number[] } {
  const walker = layer.ownerDocument.createTreeWalker(layer, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
  let raw = "";
  const nodes: Text[] = [];
  const starts: number[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (n.nodeType === Node.TEXT_NODE) {
      nodes.push(n as Text);
      starts.push(raw.length);
      raw += (n as Text).data;
    } else if ((n as Element).tagName === "BR") {
      raw += "\n";
    }
  }
  return { raw, nodes, starts };
}

function intersects(node: Node, range: Range): boolean {
  try {
    return range.intersectsNode(node);
  } catch {
    return false;
  }
}

/** Whether `range` covers all of `el`. */
function covers(range: Range, el: Element): boolean {
  try {
    return range.comparePoint(el, 0) === 0 && range.comparePoint(el, el.childNodes.length) === 0;
  } catch {
    return false;
  }
}

/** MinerU's HTML tables (`parse/mineru/render/`, a table with no crop) as pipe tables. */
function pipeTables(md: string): string {
  if (!/<table[\s>]/i.test(md)) return md;
  return md.replace(/<table[\s>][\s\S]*?<\/table>/gi, (html) => {
    const table = new DOMParser().parseFromString(html, "text/html").querySelector("table");
    const pipe = table ? elementMarkdown(table) : "";
    return pipe ? `\n\n${pipe}\n\n` : html;
  });
}

const FIELD = "input, textarea, [contenteditable='true']";

/** The selection inside `root` (the viewer's scroller) as markdown, or "" to
 *  leave the browser's own copy alone. */
export function pdfSelectionMarkdown(
  selection: Selection | null,
  root: HTMLElement,
  pages: ReadonlyMap<number, string>,
  resolveImage: (src: string) => string,
): string {
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return "";
  if (root.ownerDocument.activeElement?.closest(FIELD)) return "";
  const range = selection.getRangeAt(0);
  if (!root.contains(range.commonAncestorContainer)) return "";

  const sel: PageSelection[] = [];
  for (const page of Array.from(root.querySelectorAll(".page[data-page-number]"))) {
    if (!intersects(page, range)) continue;
    const n = Number(page.getAttribute("data-page-number"));
    const layer = page.querySelector(".textLayer");
    const text = layer ? layerText(layer) : null;
    if (covers(range, page)) {
      sel.push({ page: n, layer: text?.raw ?? null, start: null, end: null });
      continue;
    }
    if (!text) continue;
    let start = -1;
    let end = -1;
    text.nodes.forEach((node, k) => {
      if (!intersects(node, range)) return;
      const from = text.starts[k] + (node === range.startContainer ? range.startOffset : 0);
      const to = text.starts[k] + (node === range.endContainer ? range.endOffset : node.data.length);
      if (start < 0) start = from;
      end = to;
    });
    if (start >= 0) sel.push({ page: n, layer: text.raw, start, end });
  }
  if (!sel.length || !pages.size) return "";
  const md = markdownOfSelection(parsedDoc(pages), sel, resolveImage);
  return md && pipeTables(md).replace(/\n{3,}/g, "\n\n").trim();
}

function markdownFor(
  target: EventTarget | null,
  root: HTMLElement,
  pages: ReadonlyMap<number, string>,
  resolveImage: (src: string) => string,
): string {
  if (target instanceof Element && target.closest(FIELD)) return "";
  return pdfSelectionMarkdown(window.getSelection(), root, pages, resolveImage);
}

/** Bind as `onCopyCapture` on the scroller, so the markdown is written before
 *  any handler inside the pages runs; stopping it keeps them from overwriting
 *  ours. */
export function copyPdfAsMarkdown(
  e: ClipboardEvent,
  root: HTMLElement,
  pages: ReadonlyMap<number, string>,
  resolveImage: (src: string) => string,
): void {
  const md = markdownFor(e.target, root, pages, resolveImage);
  if (!md) return;
  e.clipboardData.setData("text/plain", md);
  e.preventDefault();
  e.stopPropagation();
}

/** The same, dragged out. No `preventDefault` — see docs/ui.md (WebKit drag). */
export function dragPdfAsMarkdown(
  e: DragEvent,
  root: HTMLElement,
  pages: ReadonlyMap<number, string>,
  resolveImage: (src: string) => string,
): void {
  const md = markdownFor(e.target, root, pages, resolveImage);
  if (md) e.dataTransfer.setData("text/plain", md);
}
