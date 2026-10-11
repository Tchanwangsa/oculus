/**
 * The Markdown view's side of the block sync: rehype plugins tag the rendered
 * document's top-level elements with their `data-page` / `data-block`
 * (`pdfBlocks.ts`), which the scroll position is read back from.
 */
import type { Element, ElementContent, Root } from "hast";
import type { VFile } from "vfile";
import { refAt, type BlockDoc, type BlockRef } from "@/lib/pdf/pdfBlocks";

const DATA = "blockAnchors";

function tag(el: Element, ref: BlockRef | null) {
  if (!ref) return;
  el.properties.dataPage = String(ref.page);
  if (ref.block != null) el.properties.dataBlock = String(ref.block);
}

const isElement = (n: ElementContent | Root["children"][number]): n is Element => n.type === "element";

/**
 * Two rehype plugins that mark each top-level element (and each item of a
 * top-level list, since MinerU files list items as blocks of their own) with
 * the page and block its source starts in. `before` runs ahead of KaTeX,
 * which swaps a display formula's `<pre>` for nodes without a position;
 * `after` runs last and gives those the tag `before` saw at the same index.
 */
export function blockAnchorPlugins(doc: BlockDoc) {
  const refOf = (el: Element) => {
    const offset = el.position?.start.offset;
    return offset == null ? null : refAt(doc, offset);
  };
  const before = () => (tree: Root, file: VFile) => {
    const seen: (BlockRef | null)[] = [];
    for (const node of tree.children) {
      if (!isElement(node)) continue;
      const ref = refOf(node);
      seen.push(ref);
      tag(node, ref);
      if (node.tagName === "ul" || node.tagName === "ol")
        for (const li of node.children) if (isElement(li) && li.tagName === "li") tag(li, refOf(li));
    }
    file.data[DATA] = seen;
  };
  const after = () => (tree: Root, file: VFile) => {
    const seen = file.data[DATA] as (BlockRef | null)[] | undefined;
    const top = tree.children.filter(isElement);
    if (!seen || seen.length !== top.length) return;
    top.forEach((el, i) => {
      if (el.properties.dataPage == null) tag(el, seen[i]);
    });
  };
  return { before, after };
}

/** The `[data-page]` attributes of an element as a ref. */
export function refOfElement(el: HTMLElement | null): BlockRef | null {
  if (!el) return null;
  const page = Number(el.dataset.page);
  if (!Number.isInteger(page)) return null;
  const block = el.dataset.block != null ? Number(el.dataset.block) : undefined;
  return { page, block: Number.isInteger(block) ? block : undefined };
}
