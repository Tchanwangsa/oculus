import type { Box } from "@/lib/maths/geometry";

/** Px between the caret and the list. */
export const GAP = 4;

/**
 * Where the list goes, in viewport px: under the caret, its left edge at
 * the caret's, flipped above when it would pass the bottom of `bounds` and
 * there is more room above, and slid left or right to stay inside them.
 */
export function placeList(
  caret: Box,
  size: { width: number; height: number },
  bounds: Box,
): { left: number; top: number; above: boolean } {
  const below = caret.bottom + GAP;
  const roomBelow = bounds.bottom - below;
  const roomAbove = caret.top - GAP - bounds.top;
  const above = size.height > roomBelow && roomAbove > roomBelow;
  const top = above ? caret.top - GAP - size.height : below;
  const left = Math.max(bounds.left, Math.min(caret.left, bounds.right - size.width));
  return { left, top, above };
}

/** The viewport box `el` shows inside: the window's, cut by each clipping
 *  or scrolling ancestor's (the editor's scroller among them). */
export function visibleBounds(el: HTMLElement): Box {
  const root = el.ownerDocument.documentElement;
  const out: Box = { left: 0, top: 0, right: root.clientWidth, bottom: root.clientHeight };
  for (let n = el.parentElement; n && n !== root; n = n.parentElement) {
    const style = getComputedStyle(n);
    const clipsX = style.overflowX !== "visible";
    const clipsY = style.overflowY !== "visible";
    if (!clipsX && !clipsY) continue;
    const r = n.getBoundingClientRect();
    if (clipsX) {
      out.left = Math.max(out.left, r.left);
      out.right = Math.min(out.right, r.right);
    }
    if (clipsY) {
      out.top = Math.max(out.top, r.top);
      out.bottom = Math.min(out.bottom, r.bottom);
    }
  }
  return out;
}
