/** Measuring holes and the portals over them, shared by the macOS clip-path
 *  (`nativeHoles.ts`) and the Windows page regions (`nativeRegions.ts`). */

/** The card's inner radius at each bottom corner the slot shares with it, so
 *  the page takes the card's rounded corners; a corner inside the card (a
 *  split pane's inner edge) stays square. */
export function bottomRadii(slot: HTMLElement, r: DOMRect): [number, number] {
  const card = slot.closest("main");
  if (!card) return [0, 0];
  const c = card.getBoundingClientRect();
  const cs = getComputedStyle(card);
  const px = (v: string) => parseFloat(v) || 0;
  if (Math.abs(r.bottom - (c.bottom - px(cs.borderBottomWidth))) > 1) return [0, 0];
  const fit = (v: number) =>
    Math.round(Math.min(Math.max(0, v), r.width / 2, r.height) * 100) / 100;
  const left =
    Math.abs(r.left - (c.left + px(cs.borderLeftWidth))) <= 1
      ? px(cs.borderBottomLeftRadius) - px(cs.borderLeftWidth)
      : 0;
  const right =
    Math.abs(r.right - (c.right - px(cs.borderRightWidth))) <= 1
      ? px(cs.borderBottomRightRadius) - px(cs.borderRightWidth)
      : 0;
  return [fit(left), fit(right)];
}

export function overlaps(a: DOMRect, b: DOMRect): boolean {
  return (
    a.width > 0 &&
    a.height > 0 &&
    a.left < b.right &&
    a.right > b.left &&
    a.top < b.bottom &&
    a.bottom > b.top
  );
}

/** Every portal (popover, menu, dialog…) element over `page`. A portal's
 *  wrapper is an unstyled div, so its children are measured too. */
export function portalsOver(page: DOMRect): Element[] {
  const found: Element[] = [];
  for (const portal of document.body.children) {
    if (portal.id === "root" || !(portal instanceof HTMLElement)) continue;
    for (const el of [portal, ...portal.children]) {
      if (overlaps(el.getBoundingClientRect(), page)) found.push(el);
    }
  }
  return found;
}
