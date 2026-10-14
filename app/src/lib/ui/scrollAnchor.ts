/**
 * Manual scroll anchoring: WebKit has no `overflow-anchor`, so a scroller that
 * re-wraps or grows above the reader would slide the view. `pickAnchor` names
 * the node the reader is looking at and where it sits; `anchorDrift` says how
 * far it has moved since, in `scrollTop` units, for the caller to scroll by.
 *
 * Rects scale with page zoom and `scrollTop` doesn't, so every offset is
 * divided by the scroller's rect-to-layout ratio (as `reveal` in
 * `app/src/hooks/find/useDomFind.ts` does).
 */

/** How deep to follow block children under the edge before settling. */
const MAX_DEPTH = 8;

/** A row whose contents are explicitly skipped while its box remains laid out. */
export const SKIPPED_CONTENT = "data-scroll-skipped";
/** The row whose block is currently preserving the reader's position. */
export const HELD_ANCHOR = "data-scroll-anchor-held";

export type Anchor = {
  node: Element;
  /** A placeholder must be refined to the reading block after reconnecting. */
  deferred?: boolean;
  /** The node's top below the scroller's top, in layout px. */
  offset: number;
};

/** What the anchor reads off the DOM besides rects; swapped in tests. */
export type AnchorProbe = {
  display: (el: Element) => string;
  /** Whether `el` sits in a content-visibility subtree that is skipped,
   *  where rects are an estimate. */
  skipped: (el: Element) => boolean;
};

export const DOM_PROBE: AnchorProbe = {
  display: (el) => getComputedStyle(el).display,
  skipped: inSkippedSubtree,
};

/** Whether `el` is inside an explicitly skipped row or an auto-skipped box.
 *  Auto skipping must be checked separately: a box-less element (say
 *  `display: contents`) fails `checkVisibility()` either way. */
export function inSkippedSubtree(el: Element): boolean {
  const skipped = el.closest(`[${SKIPPED_CONTENT}]`);
  if (skipped && skipped !== el) return true;
  if (!el.checkVisibility) return false;
  return !el.checkVisibility({ contentVisibilityAuto: true }) && el.checkVisibility();
}

/** Rect px per layout px: page zoom scales rects but not `scrollTop`. */
export function zoomScale(rectHeight: number, layoutHeight: number): number {
  return rectHeight > 0 && layoutHeight > 0 ? rectHeight / layoutHeight : 1;
}

/** The first of `count` stacked boxes whose bottom is past `edge`, or -1.
 *  Bottoms must not decrease with the index, so this is a binary search. */
export function firstBelow(count: number, bottomAt: (i: number) => number, edge: number): number {
  let lo = 0;
  let hi = count;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (bottomAt(mid) > edge) hi = mid;
    else lo = mid + 1;
  }
  return lo < count ? lo : -1;
}

/**
 * The first row crossing or below the scroller's top, then down through the
 * block child crossing that edge, so a long reply anchors at the paragraph
 * being read and a re-wrap above it in the same reply doesn't move it. A row
 * that crosses the edge while skipped has an estimated height, so the row
 * after it anchors instead.
 */
export function pickAnchor(
  scroller: HTMLElement,
  rows: ArrayLike<Element>,
  probe: AnchorProbe = DOM_PROBE,
): Anchor | null {
  if (!rows.length) return null;
  const box = scroller.getBoundingClientRect();
  const edge = box.top;
  let i = firstBelow(rows.length, (k) => rows[k].getBoundingClientRect().bottom, edge);
  if (i < 0) return null;
  let node = rows[i];
  let rect = node.getBoundingClientRect();
  if (rect.top < edge && i + 1 < rows.length && skippedContent(node, probe)) {
    node = rows[++i];
    rect = node.getBoundingClientRect();
  }
  for (let depth = 0; depth < MAX_DEPTH && rect.top < edge && !skippedContent(node, probe); depth++) {
    let next: Element | null = null;
    let nextRect = rect;
    for (const child of Array.from(node.children)) {
      const r = child.getBoundingClientRect();
      if (r.height > 0 && r.bottom > edge) {
        next = child;
        nextRect = r;
        break;
      }
    }
    // Inline boxes move sideways as lines re-wrap; their block is the anchor.
    if (!next || probe.display(next).startsWith("inline")) break;
    node = next;
    rect = nextRect;
  }
  const scale = zoomScale(box.height, scroller.offsetHeight);
  return { node, offset: (rect.top - box.top) / scale, deferred: node.hasAttribute?.(SKIPPED_CONTENT) ?? false };
}

/** How far `anchor` has moved down since it was picked, in `scrollTop` units;
 *  null once it has left `within` or been skipped, so the caller re-picks. */
export function anchorDrift(
  scroller: HTMLElement,
  anchor: Anchor,
  within: Element,
  probe: AnchorProbe = DOM_PROBE,
): number | null {
  const { node } = anchor;
  if (!node.isConnected || !within.contains(node) || probe.skipped(node)) return null;
  const box = scroller.getBoundingClientRect();
  const scale = zoomScale(box.height, scroller.offsetHeight);
  return (node.getBoundingClientRect().top - box.top) / scale - anchor.offset;
}

/** A row's own content is skipped (not the row: its box is still laid out). */
function skippedContent(row: Element, probe: AnchorProbe): boolean {
  const first = row.firstElementChild;
  return first != null && probe.skipped(first);
}

/** The nearest ancestor that scrolls vertically, or null for the viewport. */
export function scrollParent(el: Element): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const y = getComputedStyle(p).overflowY;
    if (y === "auto" || y === "scroll" || y === "overlay") return p;
  }
  return null;
}
